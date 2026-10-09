//! What a sync does with a departed file (spec: EP-15): a file this device
//! materialized whose Entry has left the Library is never carried back in. An
//! unedited one goes to the trash and its row is forgotten; an edited one is
//! kept and reported by every run; a trash that refuses leaves the file for the
//! next run.

use std::path::PathBuf;

use coffret_model::{ContainerKind, ContentHash, EntryPath};

use crate::delete::{delete_entries, DeleteRequest, DeleteSelection};
use crate::descent_error::DescentError;
use crate::device_state::{BatchId, LocalEntryState, LocalObservation, Mapping, RootMarkerId};
use crate::entry_paths::entry_path;
use crate::freeze::{freeze_folder, FreezeRequest};
use crate::in_memory_index::InMemoryIndex;
use crate::index::Index;
use crate::local_operation::LocalOperation;
use crate::root_refused::RootRefused;
use crate::sync::{sync_folders, Surfaced, SyncOutcome};
use crate::sync_conformance::fixtures::{
    at, keys, plant, policy, request, spool_dir, touch, write, NEWER,
};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// The identity the cases register their folder's root under (spec: EP-13).
fn registered() -> RootMarkerId {
    RootMarkerId::from_bytes([0x0e, 0x01, 0x05, 0x00, 0x15, 0x0e, 0x01, 0x05])
}

/// Maps the fixture's folder onto the Library root the way recording a mapping
/// does: a marker in the root, and the same identity expected of it.
///
/// A move to the trash is a write into the mapped folder, and a write is only
/// ever made through a root that vouches for itself (spec: EP-13), so every case
/// here needs a registered root rather than one that merely exists.
async fn map_registered(fixture: &SyncUnderTest) {
    fixture.fs().plant_marker(fixture.folder(), &registered());
    fixture
        .index()
        .set_mapping(Mapping::new(None, fixture.folder().to_path_buf()).expecting(registered()))
        .await
        .expect("recording a mapping must succeed");
}

/// One sync run, which the case expects to succeed.
async fn sync(fixture: &SyncUnderTest, run: i64) -> SyncOutcome {
    let keys = keys();
    sync_folders(request(
        fixture.store(),
        fixture.index(),
        &keys,
        fixture.fs(),
        run,
    ))
    .await
    .unwrap_or_else(|error| panic!("sync run {run} must succeed: {error}"))
}

/// Takes `path` out of the Library through `index`, the way a deletion asked
/// for on whichever device holds that catalog does.
async fn delete(fixture: &SyncUnderTest, index: &dyn Index, path: &str, run: i64) {
    let keys = keys();
    let outcome = delete_entries(
        DeleteRequest::new(
            fixture.store(),
            index,
            &keys,
            fixture.fs(),
            spool_dir(),
            DeleteSelection::paths([entry_path(path)].into()),
            BatchId::new(format!("delete-{run}")),
            at(run),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("deleting {path} must succeed: {error}"));
    assert_eq!(outcome.removed.len(), 1, "the Entry's Container is removed");
}

/// Uploads one file through a first sync and returns where it stands on disk.
async fn materialized(fixture: &SyncUnderTest, name: &str, content: &[u8]) -> PathBuf {
    map_registered(fixture).await;
    let path = write(fixture.fs(), fixture.folder(), name, content);
    let first = sync(fixture, 1).await;
    assert_eq!(first.added.len(), 1, "the file is carried in");
    path
}

/// This device's row for one path, if it holds one.
async fn row(index: &dyn Index, path: &str) -> Option<(LocalEntryState, Option<ContentHash>)> {
    index
        .local_entry_at(&entry_path(path))
        .await
        .expect("asking the Index for a row must succeed")
        .map(|local| (local.state, local.observation.hash))
}

/// The paths a run reported as moved to the trash, kept, or refused by the
/// trash, in that order of kinds.
fn departed(outcome: &SyncOutcome) -> (Vec<&EntryPath>, Vec<&EntryPath>, Vec<&EntryPath>) {
    let mut moved = Vec::new();
    let mut kept = Vec::new();
    let mut refused = Vec::new();
    for surfaced in &outcome.surfaced {
        match surfaced {
            Surfaced::MovedToTrash { path } => moved.push(path),
            Surfaced::KeptEdited { path } => kept.push(path),
            Surfaced::MoveToTrashRefused { path, .. } => refused.push(path),
            Surfaced::PackResident { .. } | Surfaced::DeletedLocally { .. } => {}
        }
    }
    (moved, kept, refused)
}

/// Asserts the run uploaded and committed nothing.
fn uploaded_nothing(outcome: &SyncOutcome) {
    assert!(outcome.added.is_empty(), "nothing is carried back in");
    assert!(outcome.commit.is_none(), "and nothing is committed");
}

/// An unedited file whose Entry this device deleted goes to the trash, and its
/// row with it (spec: EP-15).
pub async fn an_unedited_file_this_device_deleted_goes_to_the_trash(fixture: &SyncUnderTest) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;

    let outcome = sync(fixture, 3).await;

    uploaded_nothing(&outcome);
    let (moved, kept, refused) = departed(&outcome);
    assert_eq!(
        moved,
        [&entry_path("a.jpg")],
        "the move is reported with the path"
    );
    assert!(kept.is_empty() && refused.is_empty());
    assert_eq!(
        fixture.fs().moved_to_trash(),
        std::slice::from_ref(&path),
        "the file went to the trash"
    );
    assert!(!fixture.fs().holds(&path), "and is no longer in the folder");
    assert_eq!(
        row(fixture.index(), "a.jpg").await,
        None,
        "the row is forgotten: the path is outside this device's scope again",
    );

    let again = sync(fixture, 4).await;
    uploaded_nothing(&again);
    assert!(again.surfaced.is_empty(), "and nothing more is said of it");
}

/// The same for an Entry another device deleted: catching up is what tells
/// this device, and the copy is moved the same way (spec: EP-15).
pub async fn an_unedited_file_another_device_deleted_goes_to_the_trash(fixture: &SyncUnderTest) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    let elsewhere = InMemoryIndex::new();
    delete(fixture, &elsewhere, "a.jpg", 2).await;

    let outcome = sync(fixture, 3).await;

    uploaded_nothing(&outcome);
    let (moved, _, _) = departed(&outcome);
    assert_eq!(moved, [&entry_path("a.jpg")]);
    assert_eq!(fixture.fs().moved_to_trash(), [path]);
    assert_eq!(row(fixture.index(), "a.jpg").await, None);
}

/// A departed file whose content changed is kept, never uploaded, and reported
/// by this run and the next (spec: EP-15).
pub async fn an_edited_departed_file_is_kept_and_reported_every_run(fixture: &SyncUnderTest) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    fixture
        .fs()
        .write_file(&path, b"what the person wrote since");
    touch(fixture.fs(), &path, NEWER);

    for run in [3, 4] {
        let outcome = sync(fixture, run).await;

        uploaded_nothing(&outcome);
        let (moved, kept, refused) = departed(&outcome);
        assert_eq!(
            kept,
            [&entry_path("a.jpg")],
            "run {run} reports the kept file"
        );
        assert!(moved.is_empty() && refused.is_empty());
        assert!(
            fixture.fs().moved_to_trash().is_empty(),
            "nothing went to the trash"
        );
        assert_eq!(
            fixture.fs().content(&path).as_deref(),
            Some(&b"what the person wrote since"[..]),
            "the person's change is where they left it",
        );
        assert!(
            matches!(
                row(fixture.index(), "a.jpg").await,
                Some((LocalEntryState::Present, _))
            ),
            "the row stays, which is what reports the file again",
        );
    }
}

/// A departed file that was touched — its length or modification time moved —
/// and still hashes to what this device recorded is unedited (spec: EP-15).
pub async fn a_touched_departed_file_with_its_recorded_content_is_unedited(
    fixture: &SyncUnderTest,
) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    touch(fixture.fs(), &path, NEWER);

    let outcome = sync(fixture, 3).await;

    uploaded_nothing(&outcome);
    let (moved, kept, _) = departed(&outcome);
    assert_eq!(
        moved,
        [&entry_path("a.jpg")],
        "the content is what was recorded"
    );
    assert!(kept.is_empty());
    assert_eq!(fixture.fs().moved_to_trash(), [path]);
}

/// A row with no recorded hash — one an older build wrote — leaves nothing to
/// hold a changed file against, so the file is kept rather than guessed about
/// (spec: EP-15).
pub async fn a_departed_file_with_no_recorded_hash_is_kept_once_it_changed(
    fixture: &SyncUnderTest,
) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    let recorded = fixture
        .index()
        .local_entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a row must succeed")
        .expect("the first sync recorded the file present");
    fixture
        .index()
        .mark_present(LocalObservation {
            hash: None,
            ..recorded.observation
        })
        .await
        .expect("recording a materialization must succeed");
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    // Only the stamp moves: the content is the same, which no hash recorded
    // can now confirm.
    touch(fixture.fs(), &path, NEWER);

    let outcome = sync(fixture, 3).await;

    uploaded_nothing(&outcome);
    let (moved, kept, _) = departed(&outcome);
    assert_eq!(kept, [&entry_path("a.jpg")]);
    assert!(moved.is_empty());
    assert!(fixture.fs().holds(&path), "the file is kept");
}

/// A path where another device has since added a new Entry is not departed,
/// and the existing rules decide it (spec: EP-15, EP-10).
pub async fn a_path_a_new_entry_arrived_at_is_not_departed(fixture: &SyncUnderTest) {
    let content = b"the file's bytes";
    let path = materialized(fixture, "a.jpg", content).await;
    let elsewhere = InMemoryIndex::new();
    delete(fixture, &elsewhere, "a.jpg", 2).await;
    let (_, mtime) = fixture
        .fs()
        .observed(&path)
        .expect("the file is still on disk");
    let keys = keys();
    plant(
        fixture.store(),
        &elsewhere,
        &keys,
        ContainerKind::OneFile,
        "a.jpg",
        content,
        mtime,
        false,
    )
    .await;

    let outcome = sync(fixture, 3).await;

    uploaded_nothing(&outcome);
    assert!(
        outcome.surfaced.is_empty(),
        "an Entry stands at the path again, so nothing is departed: {:?}",
        outcome.surfaced,
    );
    assert_eq!(
        outcome.unchanged, 1,
        "the file is what this device last saw"
    );
    assert!(fixture.fs().moved_to_trash().is_empty());
    assert!(fixture.fs().holds(&path));
}

/// Once a departed file has gone to the trash, a file put back at the same path
/// is new: no present row stands behind it (spec: EP-15, EP-10).
pub async fn a_file_put_where_one_was_moved_to_the_trash_is_new(fixture: &SyncUnderTest) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    sync(fixture, 3).await;
    assert!(
        !fixture.fs().holds(&path),
        "the departed file went to the trash"
    );

    fixture
        .fs()
        .write_file(&path, b"a file of the person's own");
    let outcome = sync(fixture, 4).await;

    assert_eq!(outcome.added.len(), 1, "the new file is carried in");
    assert!(outcome.surfaced.is_empty());
}

/// A departed file the desktop's trash keeps inside the mapped root — a root
/// that is a volume's top, where the freedesktop.org Trash, and macOS's, put a
/// trashed file — is not carried back in by the next sync (spec: EP-15, EP-16).
///
/// The fake's trash keeps what it takes outside the folder, so the case puts
/// the copies where a desktop would: the file and its `.trashinfo` under the
/// per-user trash, and the same file under the two other names.
pub async fn a_departed_file_in_a_trash_folder_inside_the_root_is_not_carried_back(
    fixture: &SyncUnderTest,
) {
    let content = b"the file's bytes";
    materialized(fixture, "a.jpg", content).await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    let trashed = sync(fixture, 3).await;
    let (moved, _, _) = departed(&trashed);
    assert_eq!(
        moved,
        [&entry_path("a.jpg")],
        "the departed file went to the trash"
    );

    for relative in [
        ".Trash-1000/files/a.jpg",
        ".Trash/1000/files/a.jpg",
        ".Trashes/501/a.jpg",
    ] {
        write(fixture.fs(), fixture.folder(), relative, content);
    }
    write(
        fixture.fs(),
        fixture.folder(),
        ".Trash-1000/info/a.jpg.trashinfo",
        b"[Trash Info]\nPath=a.jpg\n",
    );

    let outcome = sync(fixture, 4).await;

    uploaded_nothing(&outcome);
    assert!(
        outcome.surfaced.is_empty(),
        "nothing in the trash is any of the run's business: {:?}",
        outcome.surfaced,
    );
}

/// A departed path whose file is gone as well holds nothing to report: the row
/// is forgotten and no deletion is inferred (spec: EP-15).
pub async fn a_departed_path_whose_file_is_gone_is_forgotten(fixture: &SyncUnderTest) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    fixture.fs().remove_file(&path);

    let outcome = sync(fixture, 3).await;

    uploaded_nothing(&outcome);
    assert!(
        outcome.surfaced.is_empty(),
        "no deletion of an Entry the Library no longer holds: {:?}",
        outcome.surfaced,
    );
    assert_eq!(row(fixture.index(), "a.jpg").await, None);
}

/// A trash that refuses leaves the file and its row, reports what it said,
/// fails nothing else, and is asked again by the next run (spec: EP-15).
pub async fn a_refused_move_to_the_trash_leaves_the_file_and_is_retried(fixture: &SyncUnderTest) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    let other = write(fixture.fs(), fixture.folder(), "b.jpg", b"a new file");
    fixture.fs().fail_on(LocalOperation::MovingToTrash, 1);

    let refused = sync(fixture, 3).await;

    let (moved, kept, failed) = departed(&refused);
    assert!(moved.is_empty() && kept.is_empty());
    assert_eq!(
        failed,
        [&entry_path("a.jpg")],
        "the refusal is reported with the path"
    );
    assert!(
        refused.surfaced.iter().any(|surfaced| matches!(
            surfaced,
            Surfaced::MoveToTrashRefused { cause, .. }
                if matches!(cause.as_ref(), DescentError::Io(io)
                    if matches!(io.operation, LocalOperation::MovingToTrash))
        )),
        "with the reason it was refused for",
    );
    assert!(fixture.fs().holds(&path), "the file is left where it was");
    assert!(
        matches!(
            row(fixture.index(), "a.jpg").await,
            Some((LocalEntryState::Present, _))
        ),
        "and its row with it",
    );
    assert_eq!(
        refused.added.len(),
        1,
        "the rest of the run went on: {other:?}"
    );

    let retried = sync(fixture, 4).await;

    let (moved, _, failed) = departed(&retried);
    assert_eq!(moved, [&entry_path("a.jpg")], "the next run asks again");
    assert!(failed.is_empty());
    assert_eq!(fixture.fs().moved_to_trash(), [path]);
}

/// Nothing goes to the trash through a root that does not vouch for itself:
/// the move is refused and reported, and the file stays (spec: EP-13, EP-15).
pub async fn nothing_is_moved_to_the_trash_through_a_refused_root(fixture: &SyncUnderTest) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    // Another registration's marker now stands in the root: the folder is not
    // the one this mapping was recorded against.
    fixture
        .fs()
        .plant_marker(fixture.folder(), &RootMarkerId::from_bytes([0xaa; 8]));

    let outcome = sync(fixture, 3).await;

    assert!(
        outcome.surfaced.iter().any(|surfaced| matches!(
            surfaced,
            Surfaced::MoveToTrashRefused { cause, .. }
                if matches!(cause.as_ref(), DescentError::Refused {
                    reason: RootRefused::MarkerMismatch,
                    ..
                })
        )),
        "the move is refused for the root: {:?}",
        outcome.surfaced,
    );
    assert!(fixture.fs().moved_to_trash().is_empty());
    assert!(fixture.fs().holds(&path));
}

/// Nothing goes to the trash under a root the scan could not read, and no row
/// under it is forgotten: an unplugged disk is not a folder emptied
/// (spec: EP-12, EP-15).
pub async fn nothing_is_moved_to_the_trash_under_an_unavailable_root(fixture: &SyncUnderTest) {
    let root = fixture.folder().join("disk");
    fixture.fs().create_dir(&root);
    fixture.fs().plant_marker(&root, &registered());
    fixture
        .index()
        .set_mapping(Mapping::new(None, root.clone()).expecting(registered()))
        .await
        .expect("recording a mapping must succeed");
    write(fixture.fs(), &root, "a.jpg", b"the file's bytes");
    assert_eq!(sync(fixture, 1).await.added.len(), 1);
    delete(fixture, fixture.index(), "a.jpg", 2).await;
    fixture.fs().remove_dir_all(&root);

    let outcome = sync(fixture, 3).await;

    assert_eq!(
        outcome.unavailable.len(),
        1,
        "the root is reported unavailable"
    );
    assert!(outcome.surfaced.is_empty(), "{:?}", outcome.surfaced);
    assert!(fixture.fs().moved_to_trash().is_empty());
    assert!(
        matches!(
            row(fixture.index(), "a.jpg").await,
            Some((LocalEntryState::Present, _))
        ),
        "the row is nobody's evidence while the root is away, and stays",
    );
}

/// A freeze never carries a departed file back in either: it leaves the file,
/// its row, and the trash alone, and the next sync is what moves it
/// (spec: EP-15).
pub async fn a_freeze_leaves_a_departed_file_to_the_sync(fixture: &SyncUnderTest) {
    let path = materialized(fixture, "a.jpg", b"the file's bytes").await;
    delete(fixture, fixture.index(), "a.jpg", 2).await;

    let keys = keys();
    let frozen = freeze_folder(
        FreezeRequest::new(
            fixture.store(),
            fixture.index(),
            &keys,
            fixture.fs(),
            fixture.fs(),
            spool_dir(),
            FREEZE_TARGET,
            BatchId::new("freeze-3"),
            at(3),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("the freeze must succeed: {error}"));

    assert_eq!(frozen.frozen_entries(), 0, "nothing is packed back in");
    assert!(frozen.commit.is_none(), "and nothing is committed");
    assert!(fixture.fs().holds(&path), "the file is left where it was");
    assert!(
        fixture.fs().moved_to_trash().is_empty(),
        "a freeze moves nothing to the trash"
    );
    assert!(
        matches!(
            row(fixture.index(), "a.jpg").await,
            Some((LocalEntryState::Present, _))
        ),
        "and its row stays, for the sync to read",
    );

    let outcome = sync(fixture, 4).await;

    let (moved, _, _) = departed(&outcome);
    assert_eq!(moved, [&entry_path("a.jpg")], "the sync moves it");
    assert_eq!(fixture.fs().moved_to_trash(), [path]);
}

/// A Pack size every freeze here stays under, since the case packs nothing.
const FREEZE_TARGET: u64 = 400;
