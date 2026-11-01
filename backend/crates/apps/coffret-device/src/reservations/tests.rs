//! A drop's choice of reservation, made against a catalog the case can pause
//! (spec: PK-17).
//!
//! What these hold is the order of a drop's steps against another drop's:
//! a reservation the Index has made and this process does not hold yet is
//! one in progress that nobody holds, which is what a second drop of the
//! folder takes over. So the catalog here stops a drop inside
//! [`Index::reserve`], after the reservation is made and before the call
//! returns, and the case lets the second drop run in that moment.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint, JournalRecord,
    LibraryId, MasterKey, MasterKeyEpoch, SnapshotContent,
};
use coffret_usecase::device_state::{
    AttemptedSlot, BatchId, DeviceTime, HeldParcel, LocalEntry, LocalObservation, Mapping,
    PendingRow, Reservation, ReservationId,
};
use coffret_usecase::{
    CommittedBatch, InMemoryIndex, InMemoryStore, Index, IndexResult, LibraryKeys, PendingRowsGuard,
};
use tokio::sync::oneshot;

use crate::open_library::OpenLibrary;
use crate::testing::{entry_path, local_fs};

/// A catalog that answers as [`InMemoryIndex`] does, and can be told to stop
/// the next [`Index::reserve`] once the reservation is made.
struct PausingIndex {
    inner: InMemoryIndex,
    pause: Mutex<Option<Pause>>,
}

/// One armed pause: what says the reservation is made, and what lets the call
/// return.
struct Pause {
    reserved: oneshot::Sender<()>,
    resume: oneshot::Receiver<()>,
}

/// The case's side of one armed pause.
struct Paused {
    /// Ready once the paused call has made its reservation.
    reserved: oneshot::Receiver<()>,
    /// Lets the paused call return.
    resume: oneshot::Sender<()>,
}

impl PausingIndex {
    fn new() -> Self {
        Self {
            inner: InMemoryIndex::new(),
            pause: Mutex::new(None),
        }
    }

    /// Stops the next reservation made, once it is made, until the case lets
    /// it go.
    fn pause_next_reserve(&self) -> Paused {
        let (reserved, reserved_rx) = oneshot::channel();
        let (resume_tx, resume) = oneshot::channel();
        *self.pause.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(Pause { reserved, resume });
        Paused {
            reserved: reserved_rx,
            resume: resume_tx,
        }
    }
}

#[async_trait]
impl Index for PausingIndex {
    async fn own_pending_rows(&self) -> IndexResult<PendingRowsGuard> {
        self.inner.own_pending_rows().await
    }

    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()> {
        self.inner.restore(snapshot).await
    }

    async fn apply(&self, record: JournalRecord) -> IndexResult<()> {
        self.inner.apply(record).await
    }

    async fn refresh(&self, batch: CommittedBatch) -> IndexResult<()> {
        self.inner.refresh(batch).await
    }

    async fn complete_applied(&self, batch: CommittedBatch) -> IndexResult<()> {
        self.inner.complete_applied(batch).await
    }

    async fn snapshot(&self) -> IndexResult<SnapshotContent> {
        self.inner.snapshot().await
    }

    async fn checkpoint(&self) -> IndexResult<Option<IndexCheckpoint>> {
        self.inner.checkpoint().await
    }

    async fn entry_at(&self, path: &EntryPath) -> IndexResult<Option<EntryLocation>> {
        self.inner.entry_at(path).await
    }

    async fn entries_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<EntryLocation>> {
        self.inner.entries_under(prefix).await
    }

    async fn entries_in(&self, container_id: ContainerId) -> IndexResult<Vec<EntryLocation>> {
        self.inner.entries_in(container_id).await
    }

    async fn containers_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<ContainerSummary>> {
        self.inner.containers_under(prefix).await
    }

    async fn set_mapping(&self, mapping: Mapping) -> IndexResult<()> {
        self.inner.set_mapping(mapping).await
    }

    async fn mappings(&self) -> IndexResult<Vec<Mapping>> {
        self.inner.mappings().await
    }

    async fn mark_present(&self, observation: LocalObservation) -> IndexResult<()> {
        self.inner.mark_present(observation).await
    }

    async fn mark_absent(&self, path: &EntryPath, at: DeviceTime) -> IndexResult<()> {
        self.inner.mark_absent(path, at).await
    }

    async fn forget_local_entry(&self, path: &EntryPath) -> IndexResult<()> {
        self.inner.forget_local_entry(path).await
    }

    async fn local_entry_at(&self, path: &EntryPath) -> IndexResult<Option<LocalEntry>> {
        self.inner.local_entry_at(path).await
    }

    async fn present_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<LocalEntry>> {
        self.inner.present_under(prefix).await
    }

    async fn entries_absent_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<EntryLocation>> {
        self.inner.entries_absent_under(prefix).await
    }

    async fn present_without_entry(&self) -> IndexResult<Vec<LocalEntry>> {
        self.inner.present_without_entry().await
    }

    async fn awaiting_relocation(&self) -> IndexResult<Vec<LocalEntry>> {
        self.inner.awaiting_relocation().await
    }

    async fn relocate_local_entries(
        &self,
        relocated: &[(EntryPath, EntryPath)],
    ) -> IndexResult<()> {
        self.inner.relocate_local_entries(relocated).await
    }

    async fn record_pending_row(&self, pending: PendingRow) -> IndexResult<()> {
        self.inner.record_pending_row(pending).await
    }

    async fn mark_spooled(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.mark_spooled(container_id).await
    }

    async fn mark_commit_attempted(
        &self,
        container_ids: &[ContainerId],
        slot: &AttemptedSlot,
    ) -> IndexResult<()> {
        self.inner.mark_commit_attempted(container_ids, slot).await
    }

    async fn attempted_slots(&self, batch: &BatchId) -> IndexResult<Vec<AttemptedSlot>> {
        self.inner.attempted_slots(batch).await
    }

    async fn clear_pending_row(&self, container_id: ContainerId) -> IndexResult<()> {
        self.inner.clear_pending_row(container_id).await
    }

    async fn pending_rows(&self) -> IndexResult<Vec<PendingRow>> {
        self.inner.pending_rows().await
    }

    async fn hold_parcel(&self, parcel: HeldParcel) -> IndexResult<()> {
        self.inner.hold_parcel(parcel).await
    }

    async fn held_parcels(&self) -> IndexResult<Vec<HeldParcel>> {
        self.inner.held_parcels().await
    }

    async fn let_go_parcel(&self, container_id: ContainerId, index: u64) -> IndexResult<()> {
        self.inner.let_go_parcel(container_id, index).await
    }

    async fn reserve(&self, folder: Option<&EntryPath>) -> IndexResult<ReservationId> {
        let id = self.inner.reserve(folder).await?;
        let pause = self
            .pause
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(Pause { reserved, resume }) = pause {
            let _ = reserved.send(());
            let _ = resume.await;
        }
        Ok(id)
    }

    async fn extend_reservation(&self, id: ReservationId, path: &EntryPath) -> IndexResult<()> {
        self.inner.extend_reservation(id, path).await
    }

    async fn mark_received(&self, id: ReservationId) -> IndexResult<()> {
        self.inner.mark_received(id).await
    }

    async fn reservations(&self) -> IndexResult<Vec<Reservation>> {
        self.inner.reservations().await
    }

    async fn release_reservation(&self, id: ReservationId) -> IndexResult<()> {
        self.inner.release_reservation(id).await
    }
}

/// A Library over `index` and nothing else a reservation reaches.
fn library(index: Arc<dyn Index>) -> OpenLibrary {
    OpenLibrary {
        store: Arc::new(InMemoryStore::new(64)),
        index,
        local_fs: local_fs(),
        trash: local_fs(),
        keys: LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        ),
        spool: std::env::temp_dir(),
        parcel_dir: std::env::temp_dir().join("parcels"),
        parcel_len: coffret_format::PARCEL_LEN,
        library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
        name: None,
        epoch: MasterKeyEpoch::FIRST,
        provider: "s3",
        grant: None,
        births: Default::default(),
        fetch_memory: Default::default(),
        live: Default::default(),
    }
}

/// A reservation of `folder` naming `paths`, left as a drop cut off part way
/// leaves one: made, never marked received, and held by nobody.
async fn cut_off(index: &dyn Index, folder: &EntryPath, paths: &[&str]) -> ReservationId {
    let id = index
        .reserve(Some(folder))
        .await
        .expect("making a reservation must succeed");
    for path in paths {
        index
            .extend_reservation(id, &entry_path(*path))
            .await
            .expect("extending a reservation must succeed");
    }
    id
}

fn paths(texts: &[&str]) -> BTreeSet<EntryPath> {
    texts.iter().map(|text| entry_path(*text)).collect()
}

// PK-17: a drop that has made its reservation and not yet held it, and a
// second drop of the same folder looking for one to take over in that moment,
// leave exactly one holder: the second finds the first's reservation already
// held, and takes nothing over.
#[tokio::test]
async fn two_drops_of_one_folder_racing_leave_one_holder() {
    let index = Arc::new(PausingIndex::new());
    let library = library(index.clone());
    let folder = entry_path("book");
    let Paused { reserved, resume } = index.pause_next_reserve();

    let (made, taken, ()) = tokio::join!(
        library.reserve(Some(&folder)),
        async {
            reserved
                .await
                .expect("the first drop makes its reservation");
            library.take_over(Some(&folder)).await
        },
        async {
            // Every chance for the second drop to look while the first is
            // stopped between making its reservation and holding it.
            for _ in 0..16 {
                tokio::task::yield_now().await;
            }
            let _ = resume.send(());
        },
    );

    let made = made.expect("the first drop reserves");
    let taken = taken.expect("the second drop looks");
    assert!(
        taken.is_none(),
        "the second drop takes over nothing the first holds: {:?}",
        taken.map(|(held, _)| held.id())
    );
    assert!(library.live.is_live(made.id()));
    assert!(
        library.live.try_hold(made.id()).is_none(),
        "and the first is its one holder"
    );
}

// PK-17: where two drops of one folder were cut off, the next drop of it
// takes over the earlier and folds the later into it: one reservation in
// progress is left, naming both drops' pages, and the paths the drop starts
// from are both drops'. A received reservation of the same folder, and one a
// drop in this process still holds, are neither taken over nor folded.
#[tokio::test]
async fn of_two_cut_off_reservations_the_earlier_is_taken_over_and_the_later_folded_in() {
    let index: Arc<dyn Index> = Arc::new(InMemoryIndex::new());
    let library = library(Arc::clone(&index));
    let folder = entry_path("book");
    let received = cut_off(&*index, &folder, &["book/0.jpg"]).await;
    index
        .mark_received(received)
        .await
        .expect("marking a reservation received must succeed");
    let earlier = cut_off(&*index, &folder, &["book/1.jpg", "book/2.jpg"]).await;
    let later = cut_off(&*index, &folder, &["book/2.jpg", "book/3.jpg"]).await;
    let elsewhere = cut_off(&*index, &entry_path("other"), &["other/1.jpg"]).await;
    let live = library
        .reserve(Some(&folder))
        .await
        .expect("a drop in this process reserves");

    let (held, from) = library
        .take_over(Some(&folder))
        .await
        .expect("looking for a reservation must succeed")
        .expect("a cut-off reservation is taken over");

    assert_eq!(held.id(), earlier, "the earlier one is the one continued");
    assert_eq!(
        from,
        paths(&["book/1.jpg", "book/2.jpg", "book/3.jpg"]),
        "and the drop starts from both cut-off drops' pages"
    );
    let left: Vec<(ReservationId, BTreeSet<EntryPath>)> = library
        .reservations()
        .await
        .expect("reading the reservations must succeed")
        .into_iter()
        .map(|reservation| (reservation.id, reservation.paths))
        .collect();
    assert_eq!(
        left,
        [
            (received, paths(&["book/0.jpg"])),
            (earlier, paths(&["book/1.jpg", "book/2.jpg", "book/3.jpg"])),
            (elsewhere, paths(&["other/1.jpg"])),
            (live.id(), BTreeSet::new()),
        ],
        "the later cut-off one is released once folded, and nothing else changes"
    );
    assert!(
        !library.live.is_live(later),
        "nothing holds what was folded"
    );

    drop(held);
    let again = library
        .take_over(Some(&folder))
        .await
        .expect("looking for a reservation must succeed")
        .expect("the continued one is cut off again once let go");
    assert_eq!(again.0.id(), earlier, "and is the one the next drop takes");
}
