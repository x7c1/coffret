//! The repack's contract, as tests (spec: PK-8, PK-10, PK-15, PK-23).
//!
//! The Library is put together by the flows that put one together — a freeze
//! for each Pack, a sync for a one-file Container, a committed rename — so
//! which Entries share a Pack is arranged the way it comes about in a person's
//! Library: by which invocation packed them (spec: PK-8). What a case asserts
//! about the new Packs is read off Storage the way another device reads it:
//! opened under the envelope the committed Keyring maps each to and decoded.

use std::collections::BTreeSet;

use coffret_format::{ContainerFootprint, DecodedContainer, EntryPlan, Purpose, PurposeKey};
use coffret_model::{
    ContainerId, ContainerKind, EntryLocation, EntryRename, JournalRecord, MasterKey,
    MasterKeyEpoch,
};

use super::{repack_folder, PackRefusal, RepackError, RepackOutcome, RepackRequest, Unverified};
use crate::commit::CommitPolicy;
use crate::commit_conformance::{commit_renames_in, create_library_in};
use crate::conformance_library::Library;
use crate::device_state::{BatchId, DeviceTime, Mapping};
use crate::entry_paths::entry_path;
use crate::fetch_conformance::fixtures::{container_handle, lose_key};
use crate::fetch_conformance::mangling_store::ManglingStore;
use crate::freeze::{freeze_folder, FreezeRequest, LibraryKeys};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::{sync_folders, SyncRequest};
use crate::sync_conformance::fixtures::{folder, spool_dir, spooled, touch, write, OLDER};
use crate::{InMemoryFs, InMemoryIndex, InMemoryStore};

/// A size target roomy enough that everything one run cuts fits one Pack.
const ROOMY: u64 = 64 * 1024;

fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN])
}

fn keys() -> LibraryKeys {
    LibraryKeys::derive(&master_key(), MasterKeyEpoch::FIRST)
}

fn policy() -> CommitPolicy {
    CommitPolicy::default()
        .with_replica_count(2)
        .with_checkpoint_threshold(1_000)
}

/// The clock the case's `run`th operation runs at.
fn at(run: i64) -> DeviceTime {
    DeviceTime::from_unix_seconds(1_700_000_000 + run)
}

/// Content that differs in every byte from one seed to the next.
fn filler(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// One device's Library: a store, its catalog, and its disk, the folder mapped
/// onto the Library root.
struct Device {
    store: InMemoryStore,
    index: InMemoryIndex,
    fs: InMemoryFs,
    keys: LibraryKeys,
}

impl Device {
    async fn new() -> Self {
        let device = Self {
            // Small pages, so a walk that read one page and stopped shows.
            store: InMemoryStore::new(3),
            index: InMemoryIndex::new(),
            fs: InMemoryFs::new(),
            keys: keys(),
        };
        device.fs.create_dir(folder());
        create_library_in(&device.store, device.keys.control(), &policy()).await;
        device
            .index
            .set_mapping(Mapping::new(None, folder().to_path_buf()))
            .await
            .expect("recording a mapping must succeed");
        device
    }

    /// Writes one file into the mapped folder.
    fn write(&self, relative: &str, content: &[u8]) {
        let path = write(&self.fs, folder(), relative, content);
        touch(&self.fs, &path, OLDER);
    }

    /// Packs every eligible file into one Pack, as one freeze invocation does.
    async fn freeze(&self, run: i64) -> ContainerId {
        self.freeze_under(run, ROOMY).await
    }

    /// The same, under a target of the case's own.
    async fn freeze_under(&self, run: i64, target: u64) -> ContainerId {
        let outcome = freeze_folder(
            FreezeRequest::new(
                &self.store,
                &self.index,
                &self.keys,
                &self.fs,
                &self.fs,
                spool_dir(),
                target,
                BatchId::new(format!("freeze-{run}")),
                at(run),
            )
            .with_policy(policy()),
        )
        .await
        .unwrap_or_else(|error| panic!("a freeze must succeed: {error}"));
        assert_eq!(outcome.packs.len(), 1, "a case's freeze makes one Pack");
        outcome.packs[0].container_id
    }

    /// Carries new files in one Container each.
    async fn sync(&self, run: i64) {
        sync_folders(
            SyncRequest::new(
                &self.store,
                &self.index,
                &self.keys,
                &self.fs,
                &self.fs,
                &self.fs,
                &self.fs,
                spool_dir(),
                BatchId::new(format!("sync-{run}")),
                at(run),
            )
            .with_policy(policy()),
        )
        .await
        .unwrap_or_else(|error| panic!("a sync must succeed: {error}"));
    }

    /// Commits a rename of the Entry at `from` to `to`, as another device
    /// would: the catalog catches it up at the next run.
    async fn rename(&self, from: &str, to: &str) {
        let renamed = self.current(from).await.expect("the Entry is current");
        commit_renames_in(
            &self.store,
            &self.index,
            &PurposeKey::derive(&master_key(), Purpose::ControlJournal),
            vec![
                EntryRename::new(renamed.container_id, renamed.entry_number, entry_path(to))
                    .expect("an ordinary path is a rename's target"),
            ],
        )
        .await;
    }

    /// Repacks `under` through `store`, which a case may wrap.
    async fn repack_through(
        &self,
        store: &dyn ObjectStore,
        under: &str,
        target: u64,
        run: i64,
    ) -> Result<RepackOutcome, RepackError> {
        repack_folder(
            RepackRequest::new(
                store,
                &self.index,
                &self.keys,
                &self.fs,
                spool_dir(),
                Some(entry_path(under)),
                target,
                BatchId::new(format!("repack-{run}")),
                at(run),
            )
            .with_policy(policy()),
        )
        .await
    }

    /// Repacks `under`, which the case expects to succeed.
    async fn repack(&self, under: &str, target: u64, run: i64) -> RepackOutcome {
        self.repack_through(&self.store, under, target, run)
            .await
            .unwrap_or_else(|error| panic!("a repack must succeed: {error}"))
    }

    async fn current(&self, path: &str) -> Option<EntryLocation> {
        self.index
            .entry_at(&entry_path(path))
            .await
            .expect("asking the catalog for a path must succeed")
    }

    /// The Container holding the current Entry at `path`.
    async fn holder(&self, path: &str) -> ContainerId {
        self.current(path)
            .await
            .unwrap_or_else(|| panic!("{path} is current"))
            .container_id
    }

    /// The Containers the catalog calls current.
    async fn current_containers(&self) -> BTreeSet<ContainerId> {
        self.index
            .containers_under(None)
            .await
            .expect("listing the catalog must succeed")
            .into_iter()
            .map(|summary| summary.id)
            .collect()
    }

    /// The generation the catalog stands at.
    async fn head(&self) -> u64 {
        self.index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .expect("the case committed something")
            .head_generation()
            .get()
    }

    /// Opens a Container as another device would, under `record`.
    async fn opened(&self, record: &JournalRecord, container_id: ContainerId) -> DecodedContainer {
        Library::read(&self.store)
            .await
            .open(&self.store, record, container_id, &master_key())
            .await
    }

    /// Whether Storage still lists a Container's object.
    async fn stored(&self, container_id: ContainerId) -> bool {
        Library::read(&self.store)
            .await
            .holds_container(container_id)
    }

    /// What one Pack holding exactly `paths` would measure, read off the
    /// catalog's rows the way the repack reads them.
    async fn footprint(&self, paths: &[&str]) -> u64 {
        let mut plans = Vec::new();
        for path in paths {
            let location = self.current(path).await.expect("the Entry is current");
            plans.push(EntryPlan::from(&location.entry));
        }
        ContainerFootprint::of(ContainerKind::Pack, &plans)
            .expect("a short table measures")
            .bytes()
    }
}

/// The paths a decoded Container holds, in its own order.
fn paths_of(decoded: &DecodedContainer) -> Vec<&str> {
    decoded
        .entries
        .iter()
        .map(|entry| entry.metadata.path.as_str())
        .collect()
}

// PK-23: the Packs selected are every current Pack holding an Entry under the
// folder, whole — an Entry of one outside the folder goes along — and one-file
// Containers and Packs holding nothing under it are not touched (spec: PK-1).
#[tokio::test]
async fn the_packs_holding_the_folder_are_selected_whole_and_nothing_else() {
    let device = Device::new().await;
    device.write("books/s/1.jpg", &filler(200, 1));
    device.write("other/z.jpg", &filler(200, 2));
    let mixed = device.freeze(1).await;
    device.write("books/s/3.jpg", &filler(200, 3));
    let later = device.freeze(2).await;
    device.write("music/m.flac", &filler(200, 4));
    let elsewhere = device.freeze(3).await;
    device.write("books/s/2.jpg", &filler(200, 5));
    device.sync(4).await;
    let one_file = device.holder("books/s/2.jpg").await;

    let outcome = device.repack("books/s", ROOMY, 5).await;

    assert_eq!(
        outcome.selected.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([mixed, later]),
        "the two Packs holding books/s, and neither the other Pack nor the one-file Container",
    );
    assert!(outcome.kept.is_empty());
    assert_eq!(
        outcome.removed.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([mixed, later]),
    );
    let [added] = outcome.added.as_slice() else {
        panic!("one new Pack, got {:?}", outcome.added);
    };
    let commit = outcome.commit.as_ref().expect("the repack commits");
    let decoded = device.opened(&commit.record, added.container_id).await;
    assert_eq!(
        paths_of(&decoded),
        ["books/s/1.jpg", "books/s/3.jpg", "other/z.jpg"],
        "the Entry outside the folder goes along with its Pack, in path order",
    );
    assert_eq!(device.holder("books/s/2.jpg").await, one_file);
    assert_eq!(device.holder("music/m.flac").await, elsewhere);
    assert!(device.stored(elsewhere).await && device.stored(one_file).await);
}

// PK-23, PK-3, PK-10, PK-15, CP-1, CP-14: two Packs whose path ranges
// interleave come out as Packs cut in path order under the target, one batch
// removing both and adding the new ones. Each carried Entry keeps its
// modification time, birth time, hash and deposit time, and its content byte
// for byte; the old objects go to the provider's trash (spec: OC-6).
#[tokio::test]
async fn interleaved_packs_are_cut_in_path_order_in_one_batch() {
    let device = Device::new().await;
    let files: Vec<(&str, Vec<u8>)> = vec![
        // Shrinking, so the two that follow the first cut fit under it.
        ("albums/a.jpg", filler(330, 1)),
        ("albums/b.jpg", filler(320, 2)),
        ("albums/c.jpg", filler(310, 3)),
        ("albums/d.jpg", filler(300, 4)),
    ];
    for (path, content) in files.iter().step_by(2) {
        device.write(path, content);
    }
    let odd = device.freeze(1).await;
    for (path, content) in files.iter().skip(1).step_by(2) {
        device.write(path, content);
    }
    let even = device.freeze(2).await;
    let rows_before: Vec<EntryLocation> = {
        let mut rows = Vec::new();
        for (path, _) in &files {
            rows.push(device.current(path).await.expect("frozen"));
        }
        rows
    };
    let head_before = device.head().await;
    // Exactly the first two: the cut closes there and opens a second Pack.
    let target = device.footprint(&["albums/a.jpg", "albums/b.jpg"]).await;

    let outcome = device.repack("albums", target, 3).await;

    let commit = outcome.commit.as_ref().expect("the repack commits");
    assert_eq!(
        device.head().await,
        head_before + 1,
        "one batch (spec: CP-1)"
    );
    assert_eq!(
        commit
            .record
            .removals()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([odd, even]),
        "both old Packs in removals",
    );
    let added: Vec<ContainerId> = outcome.added.iter().map(|pack| pack.container_id).collect();
    assert_eq!(
        commit
            .record
            .additions()
            .iter()
            .map(|addition| addition.container().id)
            .collect::<BTreeSet<_>>(),
        added.iter().copied().collect::<BTreeSet<_>>(),
        "the new Packs in additions, as the outcome names them",
    );
    assert_eq!(added.len(), 2);
    assert!(added.iter().all(|id| *id != odd && *id != even));

    let first = device.opened(&commit.record, added[0]).await;
    let second = device.opened(&commit.record, added[1]).await;
    assert_eq!(paths_of(&first), ["albums/a.jpg", "albums/b.jpg"]);
    assert_eq!(paths_of(&second), ["albums/c.jpg", "albums/d.jpg"]);
    for decoded in [&first, &second] {
        assert_eq!(decoded.kind, ContainerKind::Pack, "(spec: PK-15)");
        for entry in &decoded.entries {
            let path = entry.metadata.path.as_str();
            let before = rows_before
                .iter()
                .find(|row| row.entry.path.as_str() == path)
                .expect("every carried Entry was frozen");
            assert_eq!(entry.metadata.mtime, before.entry.mtime, "{path}");
            assert_eq!(entry.metadata.btime, before.entry.btime, "{path}");
            assert_eq!(entry.metadata.hash, before.entry.hash, "{path}");
            let content = &files.iter().find(|(p, _)| *p == path).expect("written").1;
            assert_eq!(&entry.content, content, "{path} byte for byte");
            let now = device.current(path).await.expect("still current");
            assert_eq!(
                now.deposited_at, before.deposited_at,
                "{path} keeps its deposit time (spec: FM-15)",
            );
        }
    }
    assert!(!device.stored(odd).await && !device.stored(even).await);
    assert_eq!(spooled(&device.fs), 0, "no copy or spool is left behind");
    assert!(device
        .index
        .pending_rows()
        .await
        .expect("asking for pending rows must succeed")
        .is_empty());
    assert_eq!(
        outcome.uploaded(),
        outcome.added.iter().map(|pack| pack.written).sum::<u64>()
    );
}

// PK-23, PK-10: Entries longer than a chunk, and Entries sharing one, are
// carried whole when the new Pack draws them from two old ones in turn: the
// copies are read across chunk boundaries, and each new Entry is held to the
// hash its row records as it passes (spec: FM-5, PK-18).
#[tokio::test]
async fn entries_across_chunk_boundaries_are_carried_whole() {
    let device = Device::new().await;
    let chunk = coffret_format::ChunkSize::DEFAULT.get() as usize;
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("albums/a.jpg", filler(chunk + chunk / 2, 1)),
        ("albums/b.jpg", filler(17, 2)),
        ("albums/c.jpg", filler(chunk / 3, 3)),
        ("albums/d.jpg", filler(2 * chunk + 5, 4)),
        ("albums/e.jpg", Vec::new()),
    ];
    let roomy = 64 * chunk as u64;
    for (path, content) in files.iter().step_by(2) {
        device.write(path, content);
    }
    device.freeze_under(1, roomy).await;
    for (path, content) in files.iter().skip(1).step_by(2) {
        device.write(path, content);
    }
    device.freeze_under(2, roomy).await;

    let outcome = device.repack("albums", roomy, 3).await;

    let commit = outcome.commit.as_ref().expect("the repack commits");
    let [added] = outcome.added.as_slice() else {
        panic!("one new Pack, got {:?}", outcome.added);
    };
    let decoded = device.opened(&commit.record, added.container_id).await;
    assert_eq!(
        paths_of(&decoded),
        files.iter().map(|(path, _)| *path).collect::<Vec<_>>(),
    );
    for ((path, content), entry) in files.iter().zip(&decoded.entries) {
        assert!(&entry.content == content, "{path} byte for byte");
    }
}

// PK-23, PK-10, FM-15: an Entry renamed into another folder is regrouped with
// that folder's Entries, and carried under its current path, which becomes
// its captured path.
#[tokio::test]
async fn a_renamed_entry_moves_into_its_new_folders_pack() {
    let device = Device::new().await;
    device.write("albums/a.jpg", &filler(4_000, 1));
    device.write("albums/b.jpg", &filler(100, 2));
    let albums = device.freeze(1).await;
    device.write("books/x.jpg", &filler(100, 3));
    let books = device.freeze(2).await;
    // Room for the two books and no more: the album on its own is over it.
    // Measured before the rename lands in the catalog, which is on the
    // repack's own catch-up, with the moved book under the path it moves to.
    let moved = device.current("albums/b.jpg").await.expect("frozen").entry;
    let shelved = device.current("books/x.jpg").await.expect("frozen").entry;
    let target = ContainerFootprint::of(
        ContainerKind::Pack,
        &[
            EntryPlan {
                path: entry_path("books/w.jpg"),
                ..EntryPlan::from(&moved)
            },
            EntryPlan::from(&shelved),
        ],
    )
    .expect("a short table measures")
    .bytes();
    device.rename("albums/b.jpg", "books/w.jpg").await;

    let outcome = device.repack("books", target, 3).await;

    assert_eq!(
        outcome.selected.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([albums, books]),
        "the album Pack holds a book now, so it is selected",
    );
    let shelf = device.holder("books/w.jpg").await;
    assert_eq!(
        device.holder("books/x.jpg").await,
        shelf,
        "the renamed book shares a Pack with the folder it moved into",
    );
    assert_ne!(device.holder("albums/a.jpg").await, shelf);
    let commit = outcome.commit.as_ref().expect("the repack commits");
    let decoded = device.opened(&commit.record, shelf).await;
    assert_eq!(paths_of(&decoded), ["books/w.jpg", "books/x.jpg"]);
    assert_eq!(
        decoded.entries[0].metadata.captured_path.as_str(),
        "books/w.jpg",
        "a rewrite captures the name current at the rewrite",
    );
    assert_eq!(decoded.entries[0].content, filler(100, 2));
}

// PK-23: a segment that is exactly one selected Pack, in its order, captured
// under its current paths, is that Pack and is kept; the rest is rewritten.
// And a repack run straight after one commits nothing, as does one over a
// folder already in order.
#[tokio::test]
async fn a_pack_already_in_place_is_kept_and_a_second_run_commits_nothing() {
    let device = Device::new().await;
    device.write("albums/a.jpg", &filler(300, 1));
    device.write("albums/b.jpg", &filler(300, 2));
    let placed = device.freeze(1).await;
    device.write("albums/c.jpg", &filler(50, 3));
    let c = device.freeze(2).await;
    device.write("albums/d.jpg", &filler(50, 4));
    let d = device.freeze(3).await;
    let target = device.footprint(&["albums/a.jpg", "albums/b.jpg"]).await;

    let outcome = device.repack("albums", target, 4).await;

    assert_eq!(
        outcome.kept,
        [placed],
        "the first Pack is already the first cut"
    );
    assert_eq!(
        outcome.removed.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([c, d]),
    );
    assert_eq!(outcome.added.len(), 1);
    let commit = outcome.commit.as_ref().expect("the rest is rewritten");
    assert!(
        !commit.record.removals().contains(&placed),
        "a kept Pack is never removed",
    );
    assert_eq!(device.holder("albums/a.jpg").await, placed);
    assert!(device.stored(placed).await);
    let rewritten = device.holder("albums/c.jpg").await;
    assert_eq!(device.holder("albums/d.jpg").await, rewritten);

    let head = device.head().await;
    let again = device.repack("albums", target, 5).await;
    assert!(again.commit.is_none(), "nothing left to do (spec: CP-1)");
    assert!(again.removed.is_empty() && again.added.is_empty());
    assert_eq!(
        again.kept.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([placed, rewritten]),
    );
    assert_eq!(device.head().await, head);
    assert_eq!(device.current_containers().await.len(), 2);
}

// PK-23, FM-15: a Pack whose order survived a rename is still not in place,
// because its meta section names the Entry by a path the Library no longer
// uses; it is rewritten, and then it is. And one whose order a rename reversed
// is rewritten in path order, its Entries read back out of their stream order.
#[tokio::test]
async fn a_pack_holding_a_renamed_entry_is_not_in_place() {
    let device = Device::new().await;
    device.write("albums/a.jpg", &filler(300, 1));
    device.write("albums/b.jpg", &filler(300, 2));
    let old = device.freeze(1).await;
    device.rename("albums/b.jpg", "albums/c.jpg").await;

    let outcome = device.repack("albums", ROOMY, 2).await;

    assert!(outcome.kept.is_empty());
    assert_eq!(outcome.removed, [old]);
    let again = device.repack("albums", ROOMY, 3).await;
    assert!(again.commit.is_none(), "rewritten once, it is in place");

    device.rename("albums/a.jpg", "albums/d.jpg").await;
    let reversed = device.repack("albums", ROOMY, 4).await;
    let commit = reversed
        .commit
        .as_ref()
        .expect("the reversed Pack is rewritten");
    let [added] = reversed.added.as_slice() else {
        panic!("one new Pack, got {:?}", reversed.added);
    };
    let decoded = device.opened(&commit.record, added.container_id).await;
    assert_eq!(paths_of(&decoded), ["albums/c.jpg", "albums/d.jpg"]);
    assert_eq!(decoded.entries[0].content, filler(300, 2));
    assert_eq!(decoded.entries[1].content, filler(300, 1));
}

// PK-23, PK-10, KL-17: a Pack to be rewritten that does not verify, or whose
// key is lost, commits nothing at all — not the other Pack's rewrite either —
// and the refusal names it. No copy, spool, or row is left behind.
#[tokio::test]
async fn an_unreadable_pack_refuses_the_whole_repack() {
    let device = Device::new().await;
    device.write("albums/a.jpg", &filler(300, 1));
    device.write("albums/c.jpg", &filler(300, 3));
    let sound = device.freeze(1).await;
    device.write("albums/b.jpg", &filler(300, 2));
    let damaged = device.freeze(2).await;
    let head = device.head().await;

    let mangling = ManglingStore::around(
        &device.store,
        container_handle(&device.store, damaged).await,
    );
    let refused = device
        .repack_through(&mangling, "albums", ROOMY, 3)
        .await
        .expect_err("a Pack that does not verify refuses the repack");
    let RepackError::Refused(refusal) = refused else {
        panic!("a refusal, got {refused:?}");
    };
    assert_eq!(refusal.container_id, damaged);
    assert!(
        matches!(
            refusal.reason,
            PackRefusal::Unverified(Unverified::CiphertextMismatch { .. })
        ),
        "{:?}",
        refusal.reason,
    );
    assert_unchanged(&device, head, &[sound, damaged]).await;

    lose_key(&device.store, &device.index, sound).await;
    let refused = device
        .repack_through(&device.store, "albums", ROOMY, 4)
        .await
        .expect_err("a key-lost Pack refuses the repack");
    let RepackError::Refused(refusal) = refused else {
        panic!("a refusal, got {refused:?}");
    };
    assert_eq!(refusal.container_id, sound);
    assert!(matches!(refusal.reason, PackRefusal::KeyLost));
    // The marker's record is the one generation since.
    assert_unchanged(&device, head + 1, &[sound, damaged]).await;
}

/// Nothing a refused repack could have done is there: no new generation, the
/// Packs current and stored, nothing in the spool, and no pending row.
async fn assert_unchanged(device: &Device, head: u64, packs: &[ContainerId]) {
    assert_eq!(device.head().await, head, "nothing committed");
    for pack in packs {
        assert!(device.current_containers().await.contains(pack));
        assert!(device.stored(*pack).await);
    }
    assert_eq!(spooled(&device.fs), 0, "no copy or spool is left behind");
    assert!(device
        .index
        .pending_rows()
        .await
        .expect("asking for pending rows must succeed")
        .is_empty());
}
