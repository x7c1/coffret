use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use coffret_model::{ControlObjectName, ObjectRef};
use tokio::sync::watch;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// How long one replica request waits for the rest of its set to arrive.
///
/// Long enough that a set sent together always meets, against a real provider
/// as much as in memory: the wait starts before the request reaches Storage, so
/// what it measures is how far apart the flow sent them, not how long Storage
/// takes. A set sent one after another never meets, and the case then fails in
/// this time rather than hanging.
const PATIENCE: Duration = Duration::from_secs(10);

/// A store that holds every Keyring replica write, and every read of one, until
/// the whole set of them is in flight, wrapped around the real one.
///
/// A commit that sends its replicas one after another cannot get past the
/// first: the first waits for company that never comes and goes on alone once
/// [`PATIENCE`] runs out, which the store notes. A commit that sends them
/// together releases all of them at once. So whether the requests overlapped
/// is a fact the store records rather than a timing a case has to infer.
///
/// It wraps whatever store the backend handed the suite, so the case runs
/// against a real provider exactly as it runs in memory.
pub(super) struct GatheringStore<'a> {
    inner: &'a dyn ObjectStore,
    puts: Gate,
    gets: Gate,
    /// What the replica writes were stored as, so that the reads of them are
    /// the ones held: a read names an object, not a name.
    replicas: Mutex<Vec<ObjectRef>>,
}

impl<'a> GatheringStore<'a> {
    /// Holds each replica write, and each read of one, until `replicas` of
    /// them are in flight.
    pub(super) fn around(inner: &'a dyn ObjectStore, replicas: usize) -> Self {
        Self {
            inner,
            puts: Gate::of(replicas),
            gets: Gate::of(replicas),
            replicas: Mutex::new(Vec::new()),
        }
    }

    /// How the replica writes arrived.
    pub(super) fn puts(&self) -> Gathered {
        self.puts.gathered()
    }

    /// How the reads of the replicas arrived.
    pub(super) fn gets(&self) -> Gathered {
        self.gets.gathered()
    }

    fn is_replica(&self, object: &ObjectRef) -> bool {
        self.replicas
            .lock()
            .expect("the gathering store's own lock is never poisoned")
            .contains(object)
    }
}

/// How one kind of replica request arrived at the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Gathered {
    /// How many arrived.
    pub(super) arrived: usize,
    /// Whether any of them gave up waiting for the rest.
    pub(super) waited_alone: bool,
}

/// Where one kind of replica request waits for the rest of its set.
struct Gate {
    expected: usize,
    arrived: watch::Sender<usize>,
    waited_alone: AtomicBool,
}

impl Gate {
    fn of(expected: usize) -> Self {
        Self {
            expected,
            arrived: watch::Sender::new(0),
            waited_alone: AtomicBool::new(false),
        }
    }

    /// Counts one request in, and returns once the whole set is in flight or
    /// once waiting for it has run out.
    async fn arrive(&self) {
        let mut seen = self.arrived.subscribe();
        self.arrived.send_modify(|arrived| *arrived += 1);
        let expected = self.expected;
        let met = matches!(
            tokio::time::timeout(PATIENCE, seen.wait_for(|arrived| *arrived >= expected)).await,
            Ok(Ok(_)),
        );
        if !met {
            self.waited_alone.store(true, Ordering::Relaxed);
        }
    }

    fn gathered(&self) -> Gathered {
        Gathered {
            arrived: *self.arrived.borrow(),
            waited_alone: self.waited_alone.load(Ordering::Relaxed),
        }
    }
}

#[async_trait]
impl ObjectStore for GatheringStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        let replica = matches!(
            ControlObjectName::parse(name),
            Ok(ControlObjectName::KeyringReplica { .. })
        );
        if replica {
            self.puts.arrive().await;
        }
        let uploaded = self.inner.put(name, body).await?;
        if replica {
            self.replicas
                .lock()
                .expect("the gathering store's own lock is never poisoned")
                .push(uploaded.object_ref.clone());
        }
        Ok(uploaded)
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
        if self.is_replica(object) {
            self.gets.arrive().await;
        }
        self.inner.get(object, range).await
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
