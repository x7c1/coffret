//! A page fetch and a read of fronts ahead, at once, over one memory.
//!
//! Listing a folder reads its Containers' fronts ahead of any fetch, and a page
//! opened while that read is still under way wants the same three things it
//! is reading: the listing, the committed key table and the Container's front
//! (spec: FM-2, PK-16). These cases hold one of those reads at Storage, start
//! the other run while it is held, and count what each asked Storage for:
//! whichever run comes second waits for the read under way rather than making
//! it again, is handed the refusal that read met, and reads for itself where
//! the reader is dropped before it lands anything.
//!
//! Not part of the backend suite: the store that holds a read is wrapped
//! around the in-memory one, and what is under test is the memory's
//! bookkeeping rather than anything a provider does.

use std::collections::BTreeMap;
use std::future::Future;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_format::Header;
use coffret_model::{ContainerId, ObjectRef};
use tokio::sync::watch;

use crate::byte_stream::ByteStream;
use crate::commit::catch_up;
use crate::commit_slot::CommitSlot;
use crate::error::{Error, Result};
use crate::fetch::{
    fetch_entry, read_fronts, EntryFetch, EntryFetchOutcome, FetchError, FetchMemory, FetchResult,
    FrontsRequest, LibraryKeys,
};
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    container_handle, entry_at, entry_request, filler, freeze_source, keys, map, policy, write,
};
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_store::InMemoryStore;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// The page every case opens, and the one beside it in the same Pack.
const PAGES: [&str; 2] = ["books/atlas/001.jpg", "books/atlas/002.jpg"];

/// What a read of Storage was for, as the case's one Container and the
/// listing tell them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// A walk of the listing, by its first page.
    Listing,
    /// Any object but the Container: in a fetch over a caught-up catalog,
    /// a replica of the committed Keyring.
    KeyTable,
    /// The Container's 32-byte header.
    Header,
    /// The meta section behind it.
    Meta,
    /// Any other range of the Container.
    Parcel,
}

/// A store that counts reads by [`Kind`], and holds the first read of one kind
/// until the case lets it go.
struct HoldingStore<'a> {
    inner: &'a dyn ObjectStore,
    container: ObjectRef,
    counts: Mutex<BTreeMap<Kind, usize>>,
    /// The kind whose first read is held, until that read arrives.
    hold: Mutex<Option<Kind>>,
    /// Whether the held read has arrived.
    reached: AtomicBool,
    /// Whether it may go on, and what it answers once it does.
    release: watch::Sender<Option<Release>>,
}

/// What the held read does once let go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Release {
    /// Reads on, as if it had never been held.
    Answer,
    /// Fails with a refusal no retry asks again after.
    Refuse,
}

impl<'a> HoldingStore<'a> {
    fn around(inner: &'a dyn ObjectStore, container: ObjectRef, hold: Kind) -> Self {
        Self {
            inner,
            container,
            counts: Mutex::new(BTreeMap::new()),
            hold: Mutex::new(Some(hold)),
            reached: AtomicBool::new(false),
            release: watch::Sender::new(None),
        }
    }

    fn count(&self, kind: Kind) -> usize {
        self.counts
            .lock()
            .expect("the case's own lock is never poisoned")
            .get(&kind)
            .copied()
            .unwrap_or(0)
    }

    fn counts(&self) -> BTreeMap<Kind, usize> {
        self.counts
            .lock()
            .expect("the case's own lock is never poisoned")
            .clone()
    }

    fn reached(&self) -> bool {
        self.reached.load(Ordering::SeqCst)
    }

    fn let_go(&self, release: Release) {
        self.release.send_replace(Some(release));
    }

    /// Counts one read, and holds it where it is the first of the kind held.
    async fn pass(&self, kind: Kind) -> Result<()> {
        *self
            .counts
            .lock()
            .expect("the case's own lock is never poisoned")
            .entry(kind)
            .or_default() += 1;
        let held = {
            let mut hold = self
                .hold
                .lock()
                .expect("the case's own lock is never poisoned");
            if *hold == Some(kind) {
                *hold = None;
                true
            } else {
                false
            }
        };
        if !held {
            return Ok(());
        }
        self.reached.store(true, Ordering::SeqCst);
        let mut release = self.release.subscribe();
        let release = *release
            .wait_for(Option::is_some)
            .await
            .expect("the store outlives every read made of it");
        match release {
            Some(Release::Answer) => Ok(()),
            Some(Release::Refuse) => Err(Error::PermissionDenied {
                detail: "the case refused the held read".to_owned(),
                source: None,
            }),
            None => unreachable!("waited for a release"),
        }
    }

    fn kind_of(&self, object: &ObjectRef, range: Option<&Range<u64>>) -> Kind {
        if object != &self.container {
            return Kind::KeyTable;
        }
        match range.map(|range| range.start) {
            Some(0) => Kind::Header,
            Some(start) if start == Header::LEN as u64 => Kind::Meta,
            _ => Kind::Parcel,
        }
    }
}

#[async_trait]
impl ObjectStore for HoldingStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        self.inner.put_if_absent(slot, body).await
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        self.inner.object_at(slot)
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        self.pass(self.kind_of(object, range.as_ref())).await?;
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        // A listing is counted, and held, at its first page: the rest are the
        // same walk going on.
        if page.is_none() {
            self.pass(Kind::Listing).await?;
        }
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}

/// One Library holding one Pack of two pages, and a target device whose
/// catalog is caught up to it and whose memory keeps nothing yet.
struct Library {
    fixture: FetchUnderTest,
    keys: LibraryKeys,
    container_id: ContainerId,
    container: ObjectRef,
}

async fn a_library() -> Library {
    let fixture = FetchUnderTest::new(
        Box::new(InMemoryStore::new(3)),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    );
    let keys = keys();
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    for (seed, page) in PAGES.iter().enumerate() {
        write(
            fixture.fs(),
            fixture.source_folder(),
            page,
            &filler(3_000, 0x50 + seed as u8),
        );
    }
    let frozen = freeze_source(&fixture, &keys, 16 * 1024 * 1024, 1).await;
    assert_eq!(frozen.packs.len(), 1, "both pages land in one Pack");
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;
    // Caught up without a memory, so the first run over the memory below is
    // the one that lists Storage and reads the key table.
    catch_up(
        fixture.store(),
        fixture.target(),
        keys.control(),
        &policy().retry,
    )
    .await
    .expect("the target device catches up");
    let container_id = entry_at(fixture.source(), PAGES[0]).await.container_id;
    let container = container_handle(fixture.store(), container_id).await;
    Library {
        fixture,
        keys,
        container_id,
        container,
    }
}

impl Library {
    fn holding(&self, kind: Kind) -> HoldingStore<'_> {
        HoldingStore::around(self.fixture.store(), self.container.clone(), kind)
    }

    async fn fetch(
        &self,
        store: &dyn ObjectStore,
        memory: &FetchMemory,
    ) -> FetchResult<EntryFetchOutcome> {
        fetch_entry(
            entry_request(store, &self.fixture, &self.keys, PAGES[0], 2).remembering(memory),
        )
        .await
    }

    async fn ahead(&self, store: &dyn ObjectStore, memory: &FetchMemory) -> FetchResult<usize> {
        let request = FrontsRequest {
            policy: policy(),
            ..FrontsRequest::new(store, self.fixture.target(), &self.keys, memory)
        };
        read_fronts(request, None, &[self.container_id]).await
    }
}

/// Drives `future` until `condition` holds, and hands it back unfinished.
///
/// The case's runs are futures on one thread rather than tasks, so a run that
/// is to wait at a held read is polled here until it has got there.
async fn until<F: Future + Unpin>(future: &mut F, condition: impl Fn() -> bool) {
    tokio::select! {
        biased;
        () = async {
            while !condition() {
                tokio::task::yield_now().await;
            }
        } => {}
        _ = future => panic!("the run finished before it reached the point the case waits for"),
    }
}

// A page opened while its folder's fronts are read ahead waits for whatever of
// that read is under way — the listing, the key table, or the front itself —
// and reads none of it again: the one front, the one listing and the one key
// table read are the read ahead's, and the fetch reads its parcel alone.
#[tokio::test]
async fn a_page_fetch_waits_for_what_the_read_ahead_is_reading() {
    for held in [Kind::Listing, Kind::KeyTable, Kind::Header] {
        let library = a_library().await;
        let store = library.holding(held);
        let memory = FetchMemory::default();

        let mut ahead = Box::pin(library.ahead(&store, &memory));
        until(&mut ahead, || store.reached()).await;
        let mut fetch = Box::pin(library.fetch(&store, &memory));
        until(&mut fetch, || memory.waiting() == 1).await;

        store.let_go(Release::Answer);
        let (ahead, fetched) = tokio::join!(ahead, fetch);
        assert_eq!(ahead.expect("the read ahead succeeds"), 1, "{held:?}");
        let fetched = fetched.expect("the page fetch succeeds");
        assert_eq!(fetched.fetch, EntryFetch::Placed, "{held:?}");

        let counts = store.counts();
        assert_eq!(counts[&Kind::Listing], 1, "one listing, held at {held:?}");
        assert_eq!(counts[&Kind::Header], 1, "one header, held at {held:?}");
        assert_eq!(counts[&Kind::Meta], 1, "one meta section, held at {held:?}");
        assert_eq!(
            counts[&Kind::KeyTable],
            1,
            "one key-table read: the first replica is whole, held at {held:?}",
        );
        assert_eq!(
            counts[&Kind::Parcel],
            1,
            "the page's parcel, held at {held:?}"
        );
    }
}

// The converse: a read ahead that finds a fetch already reading the front
// waits for it, and reads nothing of its own.
#[tokio::test]
async fn a_read_ahead_waits_for_the_front_a_fetch_is_reading() {
    let library = a_library().await;
    let store = library.holding(Kind::Header);
    let memory = FetchMemory::default();

    let mut fetch = Box::pin(library.fetch(&store, &memory));
    until(&mut fetch, || store.reached()).await;
    let before = store.counts();
    let mut ahead = Box::pin(library.ahead(&store, &memory));
    until(&mut ahead, || memory.waiting() == 1).await;
    assert_eq!(
        store.counts(),
        before,
        "the read ahead asked Storage for nothing while it waited",
    );

    store.let_go(Release::Answer);
    let (fetched, ahead) = tokio::join!(fetch, ahead);
    assert_eq!(
        fetched.expect("the page fetch succeeds").fetch,
        EntryFetch::Placed
    );
    assert_eq!(
        ahead.expect("the read ahead succeeds"),
        0,
        "the front it waited for is not one it read",
    );
    assert_eq!(store.count(Kind::Header), 1);
    assert_eq!(store.count(Kind::Meta), 1);
    assert_eq!(store.count(Kind::Listing), 1);
}

// A read that fails hands its refusal to whoever waited on it, as the reader
// met it, and the waiter does not ask Storage again on its own account.
//
// The listing and the front: a key-table read that cannot fetch one replica
// steps over it to the next (spec: KL-6), so refusing one read of it fails
// nothing.
#[tokio::test]
async fn a_failed_read_hands_its_refusal_to_its_waiters() {
    for held in [Kind::Listing, Kind::Header] {
        let library = a_library().await;
        let store = library.holding(held);
        let memory = FetchMemory::default();

        let mut ahead = Box::pin(library.ahead(&store, &memory));
        until(&mut ahead, || store.reached()).await;
        let mut fetch = Box::pin(library.fetch(&store, &memory));
        until(&mut fetch, || memory.waiting() == 1).await;

        store.let_go(Release::Refuse);
        let (_, fetched) = tokio::join!(ahead, fetch);
        let refusal = fetched.expect_err("the fetch is handed the reader's refusal");
        let storage = match &refusal {
            FetchError::Storage(error) => error,
            FetchError::Commit(crate::commit::CommitError::Storage(error)) => error,
            other => panic!("the refusal the reader met, held at {held:?}: {other:?}"),
        };
        assert!(
            matches!(storage, Error::PermissionDenied { .. }),
            "the refusal the reader met, held at {held:?}: {refusal:?}",
        );
        assert_eq!(store.count(held), 1, "read once, held at {held:?}");
    }
}

// A reader dropped while it reads — its task gone with it — leaves no waiter
// waiting on nothing: the fetch that waited reads the listing, the key table
// or the front itself.
#[tokio::test]
async fn a_dropped_reader_leaves_its_waiter_to_read_for_itself() {
    for held in [Kind::Listing, Kind::KeyTable, Kind::Header] {
        let library = a_library().await;
        let store = library.holding(held);
        let memory = FetchMemory::default();

        let mut ahead = Box::pin(library.ahead(&store, &memory));
        until(&mut ahead, || store.reached()).await;
        let mut fetch = Box::pin(library.fetch(&store, &memory));
        until(&mut fetch, || memory.waiting() == 1).await;

        drop(ahead);
        let fetched = fetch.await.unwrap_or_else(|error| {
            panic!("the page fetch reads for itself, held at {held:?}: {error:?}")
        });
        assert_eq!(fetched.fetch, EntryFetch::Placed, "{held:?}");
        assert_eq!(
            store.count(held),
            2,
            "the dropped read, and the fetch's own, held at {held:?}",
        );
        assert_eq!(
            store.count(Kind::Meta),
            1,
            "only the fetch got as far, held at {held:?}"
        );
        assert_eq!(memory.waiting(), 0, "{held:?}");
    }
}

// The debug line a page open ends on says what it read from Storage, by kind:
// everything, for a fetch nothing was read ahead of; the parcel alone, for one
// that waited on a read ahead.
#[tokio::test]
async fn a_page_open_logs_what_it_read_from_storage() {
    let logs = coffret_logging::testing::CapturedLogs::capture();
    let library = a_library().await;
    let store = library.holding(Kind::Header);
    let memory = FetchMemory::default();

    let mut ahead = Box::pin(library.ahead(&store, &memory));
    until(&mut ahead, || store.reached()).await;
    let mut fetch = Box::pin(library.fetch(&store, &memory));
    until(&mut fetch, || memory.waiting() == 1).await;
    store.let_go(Release::Answer);
    let (_, fetched) = tokio::join!(ahead, fetch);
    fetched.expect("the page fetch succeeds");

    let alone = a_library().await;
    let memory = FetchMemory::default();
    alone
        .fetch(alone.fixture.store(), &memory)
        .await
        .expect("a page fetch with nothing read ahead succeeds");

    let lines: Vec<_> = logs
        .at(tracing::Level::DEBUG)
        .into_iter()
        .filter(|event| event.message() == "what a partial fetch read from Storage")
        .collect();
    assert_eq!(lines.len(), 2, "one line for each page open");
    let reads = |line: &coffret_logging::testing::LoggedEvent| {
        [
            line.number("listings"),
            line.number("key_tables"),
            line.number("fronts"),
            line.number("parcels"),
        ]
    };
    assert_eq!(reads(&lines[0]), [0, 0, 0, 1], "waited on the read ahead");
    assert_eq!(reads(&lines[1]), [1, 1, 1, 1], "read everything itself");
    assert_eq!(lines[1].field("verdict"), "placed");
}
