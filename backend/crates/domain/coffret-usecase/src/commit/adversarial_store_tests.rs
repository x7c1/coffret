//! What a control-object read spends on a store that answers dishonestly.
//!
//! Storage is outside the trust boundary, and a control object is the one thing
//! this crate reads whole: the size of the answer is a claim, and acting on the
//! claim is spending this device's memory on it. The tag inside the object
//! settles whether the bytes were ever the Library's — afterwards. So what these
//! cases assert is not that a bad answer is refused, which the AEAD would see to
//! anyway, but that refusing it costs a bounded and small amount.
//!
//! Each one wraps the in-memory store, because these are answers no honest store
//! gives: a length nothing could be, a body that never stops, a body shorter
//! than the length beside it. A real provider can produce all three — through a
//! bug, a proxy, or somebody else with write access to the account.
//!
//! The dishonest bodies are *endless* rather than large, which is what keeps the
//! cases cheap: a reader that believed any of these claims would run until it
//! ran out of memory, and one that bounds itself hands back a verdict in
//! microseconds and can say exactly how many bytes it took to reach it.
//!
//! The last case asks the question after that one. A bound decides what a lie
//! costs; what a *flow* does with the refusal decides whether the lie is worth
//! telling. One object anybody with write access can create sits among the
//! checkpoints a catch-up walks, and the walk has to get past it (spec: CK-9).

use std::io;
use std::ops::Range;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use coffret_format::{
    encode_control_object, encode_index_snapshot, encode_journal_record, keyring_set_digest,
    max_control_object_len_at, ControlEncodeRequest, ControlHeader, ControlPayload,
    IndexSnapshotPayload,
};
use coffret_model::{
    ControlObjectKind, ControlObjectName, Generation, JournalRecord, KeyringCommitment,
    KeyringMapping, MasterKey, MasterKeyEpoch, ObjectRef, SnapshotContent,
};
use tokio::io::{AsyncRead, ReadBuf};

use super::commit_error::{CommitError, ControlObjectFault};
use super::{catch_up, control_object, ControlKeys};
use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::{Error, Result};
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::retry::RetryPolicy;

/// Objects small enough that a listing page never matters here.
const PAGE_SIZE: usize = 8;

/// How many bytes the stored object of these cases is.
const STORED_LEN: u64 = 128;

/// A body that never ends, counting what it was asked to give.
///
/// A reader that stops where it said it would takes a bounded number of bytes
/// off this and returns; one that grows to whatever arrives never returns at
/// all. The count is what lets a case state the bound rather than trust it.
struct Endless {
    handed: Arc<AtomicU64>,
}

impl AsyncRead for Endless {
    fn poll_read(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let take = buf.remaining();
        buf.initialize_unfilled_to(take);
        buf.advance(take);
        self.handed.fetch_add(take as u64, Ordering::Relaxed);
        Poll::Ready(Ok(()))
    }
}

/// How a wrapped store misreports the object it holds.
#[derive(Clone, Copy)]
enum Lie {
    /// Declares a length no control object could be, over a body that would go
    /// on forever if anyone read it.
    Declares(u64),
    /// Declares the object's real length and then does not stop.
    Overruns,
    /// Declares the object's real length and sends less than that.
    Underruns { missing: usize },
    /// Answers every range with the whole object, declaring its whole length.
    IgnoresRange,
}

/// A store that answers one way for every read.
struct LyingStore<'a> {
    inner: &'a dyn ObjectStore,
    lie: Lie,
    /// The one object the lie is told about, or `None` to tell it about every
    /// read. A Library that is sound apart from one object is the interesting
    /// case for the flows that read several: what they do with the rest is the
    /// whole question.
    only: Option<ObjectRef>,
    /// How many bytes its endless bodies have handed over.
    handed: Arc<AtomicU64>,
}

impl<'a> LyingStore<'a> {
    fn around(inner: &'a dyn ObjectStore, lie: Lie) -> Self {
        Self {
            inner,
            lie,
            only: None,
            handed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// The same store, lying about one object and answering honestly for the
    /// rest.
    fn about(inner: &'a dyn ObjectStore, object: &ObjectRef, lie: Lie) -> Self {
        Self {
            only: Some(object.clone()),
            ..Self::around(inner, lie)
        }
    }

    /// How many bytes a reader has taken off this store.
    fn handed(&self) -> u64 {
        self.handed.load(Ordering::Relaxed)
    }

    fn endless(&self, declared: u64) -> ByteStream {
        ByteStream::new(
            declared,
            Endless {
                handed: Arc::clone(&self.handed),
            },
        )
    }
}

#[async_trait]
impl ObjectStore for LyingStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
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
        if self.only.as_ref().is_some_and(|only| only != object) {
            return self.inner.get(object, range).await;
        }
        let asked = match self.lie {
            // The whole object, whatever was asked for.
            Lie::IgnoresRange => None,
            _ => range,
        };
        let bytes = self.inner.get(object, asked).await?.into_bytes().await?;
        let real = bytes.len() as u64;
        Ok(match self.lie {
            Lie::Declares(declared) => self.endless(declared),
            Lie::Overruns | Lie::IgnoresRange => self.endless(real),
            Lie::Underruns { missing } => {
                let kept = bytes.len().saturating_sub(missing);
                ByteStream::new(real, io::Cursor::new(bytes[..kept].to_vec()))
            }
        })
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}

/// The name the cases store their object under: a link in the head chain, whose
/// ceiling is as roomy as any name's, because it admits an activation Index
/// Snapshot (spec: FM-12).
fn name() -> ControlObjectName {
    ControlObjectName::head(Generation::new(4))
}

/// One attempt and no waiting: what is on trial is the first answer, and a
/// policy that retried would only ask for the same lie five more times.
fn once() -> RetryPolicy {
    RetryPolicy::default().with_attempts(1)
}

/// A store holding one object at [`name`], and the handle it is read through.
///
/// The bytes are a stand-in rather than a real control object: none of these
/// cases gets as far as opening one, which is the point.
async fn holding(store: &InMemoryStore) -> ObjectRef {
    store
        .put(
            &name().to_string(),
            ByteStream::from(vec![0x11; STORED_LEN as usize]),
        )
        .await
        .expect("writing the case's object must succeed")
}

// A declared length past what an object at that name could be is refused before
// a byte of it is read — which the endless body is what proves: a reader that
// took the declaration at its word would still be reading.
#[tokio::test]
async fn a_declared_length_past_the_ceiling_is_refused_before_any_of_it_arrives() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let object = holding(&store).await;
    let ceiling = max_control_object_len_at(&name());
    let lying = LyingStore::around(&store, Lie::Declares(ceiling + 1));

    let result = control_object::fetch(&lying, &once(), &name(), &object).await;

    let Err(Error::ObjectTooLarge {
        declared,
        ceiling: stated,
    }) = result
    else {
        panic!("expected a length past the ceiling to be refused, got {result:?}");
    };
    assert_eq!(declared, ceiling + 1);
    assert_eq!(stated, ceiling);
    assert_eq!(
        lying.handed(),
        0,
        "the claim was answered without reading anything for it",
    );
    // Nothing about the claim is worth asking a second time: it is what Storage
    // says the object is, not a transfer that went wrong.
    assert!(!Error::ObjectTooLarge {
        declared,
        ceiling: stated
    }
    .is_retryable());
}

// The ceiling is no limit on Libraries: an object well inside it is read back
// exactly, through the same call.
#[tokio::test]
async fn an_object_inside_the_ceiling_is_read_back_whole() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let object = holding(&store).await;

    let bytes = control_object::fetch(&store, &once(), &name(), &object)
        .await
        .expect("an honest answer inside the ceiling is read");
    assert_eq!(bytes, vec![0x11; STORED_LEN as usize]);
}

// An answer that runs past its own declared length is refused, and the excess is
// never buffered on the way to finding out: the reading stops one byte past the
// declaration, which is the least it takes to tell "exactly this" from "more".
#[tokio::test]
async fn an_answer_longer_than_it_declared_is_stopped_at_its_declaration() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let object = holding(&store).await;
    let lying = LyingStore::around(&store, Lie::Overruns);

    let result = control_object::fetch(&lying, &once(), &name(), &object).await;

    assert!(
        matches!(
            result,
            Err(Error::LengthOverrun {
                expected: STORED_LEN
            })
        ),
        "expected an answer declaring {STORED_LEN} bytes to be held to {STORED_LEN}, got {result:?}",
    );
    assert_eq!(
        lying.handed(),
        STORED_LEN + 1,
        "an endless answer cost one byte more than it declared, and no more",
    );
}

// And one that stops short is refused for what it is, rather than opened as a
// truncated object. This half was already true and is pinned here beside the
// other three.
#[tokio::test]
async fn an_answer_shorter_than_it_declared_is_refused() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let object = holding(&store).await;
    let lying = LyingStore::around(&store, Lie::Underruns { missing: 8 });

    let result = control_object::fetch(&lying, &once(), &name(), &object).await;

    assert!(
        matches!(
            result,
            Err(Error::LengthMismatch {
                expected: STORED_LEN,
                actual: 120,
            })
        ),
        "expected a short answer to be refused, got {result:?}",
    );
}

// The header read a commit makes before spending a slot takes its 44 bytes and
// leaves the rest. A provider that ignores the range — or an Index Snapshot of
// several megabytes answering it honestly — costs a header either way
// (spec: FM-11, CP-16).
#[tokio::test]
async fn a_header_read_takes_only_the_header() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let object = holding(&store).await;
    let lying = LyingStore::around(&store, Lie::IgnoresRange);

    // The bytes are not a control object, so the parse is what refuses them —
    // and it refuses them having been handed a header's worth.
    let result = control_object::fetch_header(&lying, &once(), &object).await;
    assert!(
        result.is_err(),
        "the case's filler bytes are no control-object header, got {result:?}",
    );
    assert!(
        lying.handed() <= ControlHeader::LEN as u64,
        "a header read took {} bytes off an endless answer",
        lying.handed(),
    );
}

// The rest of the module is one case about what a *flow* does with the refusal,
// and the small Library it needs to do it over.

/// The Master Key the catch-up case's Library works under.
fn control_keys() -> ControlKeys {
    ControlKeys::derive(
        &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
        MasterKeyEpoch::FIRST,
    )
}

/// The Keyring tuple every head in the case names (spec: CP-10).
///
/// The same empty mapping at every generation: no case here reads a Keyring,
/// and a commitment that names one is all a record and a Snapshot have to
/// carry.
fn commitment() -> KeyringCommitment {
    let digest = keyring_set_digest(&KeyringMapping::default()).expect("a mapping always digests");
    KeyringCommitment::new(Generation::FIRST, 1, &digest)
        .expect("one replica of a real digest is a commitment")
}

/// The Journal record committed at one generation, adding and removing nothing.
///
/// Empty on purpose: what the case is about is which object the walk starts
/// from, and a record carrying Containers would only make the fixture longer.
fn record_at(generation: Generation) -> JournalRecord {
    JournalRecord::new(
        generation,
        generation.get().checked_sub(1).map(Generation::new),
        MasterKeyEpoch::FIRST,
        commitment(),
        None,
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect("a fixture holds a record succeeding the head one generation back")
}

/// Seals one payload as the control object at `name` and stores it.
async fn store_control(
    store: &InMemoryStore,
    keys: &ControlKeys,
    name: &ControlObjectName,
    kind: ControlObjectKind,
    payload: &ControlPayload,
) -> ObjectRef {
    let object = encode_control_object(&ControlEncodeRequest::new(
        name,
        kind,
        keys.of_kind(kind),
        payload,
    ))
    .expect("sealing a control object under a real key must succeed");
    store
        .put(&name.to_string(), ByteStream::from(object.bytes().to_vec()))
        .await
        .expect("storing a control object must succeed")
}

/// A Library of two committed heads, each with an ordinary checkpoint over it
/// (spec: CK-10, FM-12).
async fn two_checkpointed_heads(store: &InMemoryStore, keys: &ControlKeys) {
    for generation in [Generation::FIRST, second()] {
        let record = record_at(generation);
        store_control(
            store,
            keys,
            &ControlObjectName::head(generation),
            ControlObjectKind::Journal,
            &encode_journal_record(&record).expect("an empty record encodes"),
        )
        .await;

        let content = SnapshotContent::new(record.checkpoint(), None, Vec::new(), Vec::new())
            .expect("a Library of nothing is one an Index could stand at");
        store_control(
            store,
            keys,
            &ControlObjectName::index_snapshot(generation),
            ControlObjectKind::IndexSnapshot,
            &encode_index_snapshot(&IndexSnapshotPayload::ordinary(content))
                .expect("an empty Snapshot encodes"),
        )
        .await;
    }
}

/// The newer of the case's two generations.
fn second() -> Generation {
    Generation::FIRST
        .next()
        .expect("the first generation has a successor")
}

// A checkpoint candidate whose declared length is past the ceiling is stepped
// over the way one that will not decrypt is, and the walk carries on to the
// older checkpoint (spec: CK-9).
//
// The asymmetry this pins is the one that would otherwise turn the ceiling into
// a weapon: the object at `idx-<newest>` is one anybody with write access to the
// account can create, and a length is the cheapest thing to lie about in it. If
// the refusal travelled as a Storage failure the flow reports, that one object
// would make every catch-up on this device fail — and so every commit, sync, and
// fetch — permanently, since nothing about the claim changes on a second read. A
// lying length is evidence about the object, not about the Library.
#[tokio::test]
async fn an_oversized_checkpoint_candidate_is_stepped_over() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let keys = control_keys();
    two_checkpointed_heads(&store, &keys).await;

    let newest = ControlObjectName::index_snapshot(second());
    let handle = ObjectRef::new(newest.to_string());
    let ceiling = max_control_object_len_at(&newest);
    let lying = LyingStore::about(&store, &handle, Lie::Declares(ceiling + 1));

    // The refusal itself is real, and reached before anything is read for it.
    let refused = control_object::fetch(&lying, &once(), &newest, &handle).await;
    assert!(
        matches!(refused, Err(Error::ObjectTooLarge { .. })),
        "the newest checkpoint must be refused for its declared length, got {refused:?}",
    );

    let index = InMemoryIndex::new();
    catch_up(&lying, &index, &keys, &once())
        .await
        .expect("one object declaring a lying length must not stop a catch-up");

    let standing = index
        .snapshot()
        .await
        .expect("a caught-up catalog stands somewhere");
    assert_eq!(
        standing.adopted_from(),
        Some(&ControlObjectName::index_snapshot(Generation::FIRST)),
        "the older checkpoint is what the catalog was started from",
    );
    assert_eq!(
        standing.checkpoint().head_generation(),
        second(),
        "and the record after it was replayed, leaving the catalog at the head",
    );
    assert_eq!(
        lying.handed(),
        0,
        "stepping over the claim cost nothing to read",
    );
}

// CK-10: a Snapshot checkpoints the head it is named for, and a catch-up that
// adopted one saying otherwise would leave the catalog's checkpoint and its
// recorded starting point disagreeing — every later replay reading the wrong
// one. The rule lives in the decoder, which is told the name's own generation,
// so the walk gets the refusal without making the comparison itself.
#[tokio::test]
async fn a_snapshot_that_checkpoints_another_head_is_not_adopted() {
    let store = InMemoryStore::new(PAGE_SIZE);
    let keys = control_keys();
    two_checkpointed_heads(&store, &keys).await;

    // The newest checkpoint's name says the second head; its payload says the
    // first. Only something that does not hold to CK-10 writes that.
    let content = SnapshotContent::new(
        record_at(Generation::FIRST).checkpoint(),
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect("a Library of nothing is one an Index could stand at");
    store_control(
        &store,
        &keys,
        &ControlObjectName::index_snapshot(second()),
        ControlObjectKind::IndexSnapshot,
        &encode_index_snapshot(&IndexSnapshotPayload::ordinary(content))
            .expect("an empty Snapshot encodes"),
    )
    .await;

    let index = InMemoryIndex::new();
    let refusal = catch_up(&store, &index, &keys, &once()).await.err();
    // Which refusal it was, and not merely that there was one: `Unopenable`
    // stands for everything the format layer will not hand a value back for,
    // and a case asserting on it alone would pass on a Snapshot that failed to
    // authenticate.
    assert!(
        matches!(
            refusal,
            Some(CommitError::CorruptControlObject {
                fault: ControlObjectFault::Unopenable(
                    coffret_format::Error::SnapshotCheckpointsAnotherHead {
                        generation,
                        head_generation,
                    },
                ),
                ..
            }) if generation == second() && head_generation == Generation::FIRST
        ),
        "expected a Snapshot of another head to be refused, got {refusal:?}",
    );
    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "and nothing of it restored into the catalog",
    );
}
