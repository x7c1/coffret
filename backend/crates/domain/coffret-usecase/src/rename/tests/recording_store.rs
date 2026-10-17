use std::future::Future;
use std::ops::Range;
use std::pin::Pin;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::{ContainerId, ObjectRef};

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::uploaded_object::UploadedObject;

/// Another device's commit, run the moment this store is first asked to
/// consume a slot.
pub(super) type Rival<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// A store that writes down every object a run writes and every object it
/// reads, wrapped around the real one.
///
/// What a rename is claimed not to do — upload a Container, or read one back —
/// leaves nothing behind to look at afterwards: a Container written under the
/// name it already had is the same object. So the cases count the requests.
///
/// It can also hold a rival commit back until the run under test is about to
/// consume its slot, which is the one moment another device's commit races it:
/// after the plan, after the commit's own catch-up, before the record exists
/// (spec: CP-4).
pub(super) struct RecordingStore<'a> {
    inner: &'a dyn ObjectStore,
    log: Mutex<Log>,
    rival: Mutex<Option<Rival<'a>>>,
}

/// What a run asked of the store.
#[derive(Default)]
struct Log {
    /// The name of every unconditional write.
    puts: Vec<String>,
    /// How many conditional writes, into slots of any kind.
    conditional: usize,
    /// How many objects were trashed or purged.
    removed: usize,
    /// The handle of every read.
    gets: Vec<String>,
}

impl<'a> RecordingStore<'a> {
    /// Starts recording at nothing.
    pub(super) fn around(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            log: Mutex::new(Log::default()),
            rival: Mutex::new(None),
        }
    }

    /// The same store, running `rival` before the first slot it is asked to
    /// consume.
    pub(super) fn racing(self, rival: Rival<'a>) -> Self {
        *self.rival.lock().expect("unpoisoned") = Some(rival);
        self
    }

    /// The Containers written.
    pub(super) fn containers_put(&self) -> Vec<String> {
        self.log
            .lock()
            .expect("unpoisoned")
            .puts
            .iter()
            .filter(|name| is_container(name))
            .cloned()
            .collect()
    }

    /// The Containers read, by the handle they were read through.
    pub(super) fn containers_read(&self) -> Vec<String> {
        self.log
            .lock()
            .expect("unpoisoned")
            .gets
            .iter()
            .filter(|handle| is_container(handle))
            .cloned()
            .collect()
    }

    /// How many changes of any kind: writes, conditional or not, and objects
    /// trashed or purged.
    pub(super) fn writes(&self) -> usize {
        let log = self.log.lock().expect("unpoisoned");
        log.puts.len() + log.conditional + log.removed
    }
}

/// Whether an object's name or handle is a Container's: its ID and nothing
/// else before the extension, which no control object's name is (spec: FM-3,
/// FM-12).
fn is_container(name: &str) -> bool {
    name.strip_suffix(ContainerId::STORAGE_EXTENSION)
        .is_some_and(|stem| ContainerId::from_hex(stem).is_ok())
}

#[async_trait]
impl ObjectStore for RecordingStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        self.log
            .lock()
            .expect("unpoisoned")
            .puts
            .push(name.to_owned());
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        let rival = self.rival.lock().expect("unpoisoned").take();
        if let Some(rival) = rival {
            rival.await;
        }
        self.log.lock().expect("unpoisoned").conditional += 1;
        self.inner.put_if_absent(slot, body).await
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        self.inner.object_at(slot)
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        self.log
            .lock()
            .expect("unpoisoned")
            .gets
            .push(object.as_str().to_owned());
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.log.lock().expect("unpoisoned").removed += 1;
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.log.lock().expect("unpoisoned").removed += 1;
        self.inner.purge(object).await
    }
}
