use coffret_model::{ContainerKind, ControlObjectName, EntryLocation, JournalRecord};

use crate::commit::{
    catch_up, commit_batch, CommitError, CommitOutcome, ControlKeys, PreparedBatch,
};
use crate::commit_conformance::create_library_in;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, path, policy, prepared, request,
};
use crate::commit_conformance::library::Library;
use crate::generations::generation;
use crate::in_memory_index::InMemoryIndex;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// Large enough that no listing here needs a second page.
const PAGE_SIZE: usize = 64;

/// The head every case's batches are prepared against: the Library's first
/// head, then the commit that put the case's files in place.
pub(super) const BASE: u64 = 1;

/// One Library and the two devices that write to it.
pub(super) struct Devices {
    pub(super) store: InMemoryStore,
    pub(super) keys: ControlKeys,
    /// The device whose batch is rebased.
    pub(super) ours: InMemoryIndex,
    /// The device that commits first.
    pub(super) theirs: InMemoryIndex,
}

impl Devices {
    /// A Library whose head `BASE` added each of `files` — a Container of
    /// `kind` holding Entries at the paths given, under the seed given — as
    /// `ours` committed it.
    pub(super) async fn holding(files: &[(u8, ContainerKind, &[&str])]) -> Self {
        let devices = Self {
            store: InMemoryStore::new(PAGE_SIZE),
            keys: control_keys(),
            ours: InMemoryIndex::new(),
            theirs: InMemoryIndex::new(),
        };
        create_library_in(&devices.store, &devices.keys, &policy()).await;
        let mut additions = Vec::new();
        for (seed, kind, paths) in files {
            devices.upload(*seed).await;
            additions.push(prepared(*seed, *kind, paths));
        }
        let first = devices
            .commit(&devices.ours, PreparedBatch::adding(additions))
            .await
            .expect("the commit that puts the files in place must succeed");
        assert_eq!(first.record.generation(), generation(BASE));
        devices
    }

    /// Puts Container `seed` on Storage, as its producer's upload would.
    pub(super) async fn upload(&self, seed: u8) {
        Library::upload_container(&self.store, container_id(seed)).await;
    }

    /// Commits `batch`, prepared against `BASE`, from `index`.
    pub(super) async fn commit(
        &self,
        index: &InMemoryIndex,
        batch: PreparedBatch,
    ) -> Result<CommitOutcome, CommitError> {
        self.commit_through(&self.store, index, batch).await
    }

    /// The same, through `store`.
    pub(super) async fn commit_through(
        &self,
        store: &dyn ObjectStore,
        index: &InMemoryIndex,
        batch: PreparedBatch,
    ) -> Result<CommitOutcome, CommitError> {
        let batch = batch.prepared_at(Some(generation(BASE)));
        commit_batch(request(store, index, &self.keys, batch))
            .await
            .map_err(|failure| *failure.error)
    }

    /// `theirs` commits `batch`, which has to succeed.
    pub(super) async fn rival(&self, batch: PreparedBatch) -> JournalRecord {
        self.commit(&self.theirs, batch)
            .await
            .unwrap_or_else(|error| panic!("the rival commits first and must succeed: {error:?}"))
            .record
    }

    /// The catalog a device that has never seen the Library reaches by
    /// catching up with it: what every device converges on.
    pub(super) async fn catalog(&self) -> Vec<EntryLocation> {
        let fresh = InMemoryIndex::new();
        catch_up(&self.store, &fresh, &self.keys, &policy().retry)
            .await
            .expect("catching up with the Library must succeed");
        fresh
            .entries_under(None)
            .await
            .expect("listing the catalog must succeed")
    }

    /// Whether the Library holds a head at `number`.
    pub(super) async fn has_head(&self, number: u64) -> bool {
        Library::read(&self.store)
            .await
            .holds(&ControlObjectName::head(generation(number)))
    }

    /// The Entry `index` holds at `text`, which has to be there.
    pub(super) async fn at(&self, index: &InMemoryIndex, text: &str) -> EntryLocation {
        index
            .entry_at(&path(text))
            .await
            .expect("asking the Index for a path must succeed")
            .unwrap_or_else(|| panic!("{text} holds an Entry"))
    }

    /// Whether `index` holds no Entry at `text`.
    pub(super) async fn vacant(&self, index: &InMemoryIndex, text: &str) -> bool {
        index
            .entry_at(&path(text))
            .await
            .expect("asking the Index for a path must succeed")
            .is_none()
    }
}
