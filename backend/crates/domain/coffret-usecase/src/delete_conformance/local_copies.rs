//! What a deletion does with the deleting device's own copies of the Entries
//! it deleted, right after its commit (spec: EP-15): what the next sync would
//! otherwise do with them, decided by the same rule under the same root checks
//! (spec: EP-12, EP-13), and never at the cost of the commit.

use coffret_model::EntryPath;

use crate::delete::DeleteOutcome;
use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::delete_conformance::fixtures::{
    current, delete_with_copies, folder, map_registered, paths, registered, sync, write,
};
use crate::descent_error::DescentError;
use crate::device_state::{LocalEntryState, Mapping, RootMarkerId};
use crate::entry_paths::entry_path;
use crate::index::Index;
use crate::local_operation::LocalOperation;
use crate::root_refused::RootRefused;
use crate::sync::Surfaced;
use crate::sync_conformance::fixtures::{touch, NEWER};
use crate::unavailable_root::RootUnavailable;

/// The paths the run reported as discarded, kept because edited, and
/// refused, in that order of kinds.
fn reported(outcome: &DeleteOutcome) -> (Vec<&EntryPath>, Vec<&EntryPath>, Vec<&EntryPath>) {
    let mut discarded = Vec::new();
    let mut kept = Vec::new();
    let mut refused = Vec::new();
    for surfaced in &outcome.local {
        match surfaced {
            Surfaced::Discarded { path } => discarded.push(path),
            Surfaced::KeptEdited { path } => kept.push(path),
            Surfaced::DiscardRefused { path, .. } => refused.push(path),
            Surfaced::PackResident { .. }
            | Surfaced::DeletedLocally { .. }
            | Surfaced::RenameRefused { .. } => {
                panic!("a deletion reports nothing but what became of its copies: {surfaced:?}")
            }
        }
    }
    (discarded, kept, refused)
}

/// Whether this device still holds a row saying it put a file at `path`.
async fn present(index: &dyn Index, path: &str) -> bool {
    index
        .local_entry_at(&entry_path(path))
        .await
        .expect("asking the Index for a row must succeed")
        .is_some_and(|local| local.state == LocalEntryState::Present)
}

/// Whether this device holds any row for `path` at all.
async fn any_row(index: &dyn Index, path: &str) -> bool {
    index
        .local_entry_at(&entry_path(path))
        .await
        .expect("asking the Index for a row must succeed")
        .is_some()
}

/// EP-15: the deleting device's unedited copies go to the trash as soon as the
/// deletion commits, their rows are forgotten, and the folders the discards
/// emptied are removed — never the mapped root, and never a folder that still
/// holds anything.
///
/// The next sync then finds nothing to say: the paths are outside this
/// device's scope again.
pub async fn the_deleting_devices_unedited_copies_go_to_the_trash_after_the_commit(
    fixture: &DeleteUnderTest,
) {
    let index = fixture.index();
    map_registered(fixture).await;
    let a = write(fixture, "album/sub/a.jpg", b"the first file");
    let b = write(fixture, "album/sub/b.jpg", b"the second file");
    let c = write(fixture, "album/c.jpg", b"the third file");
    let beside = write(fixture, "d.jpg", b"a file beside the folder");
    sync(fixture, 1).await;

    let outcome = delete_with_copies(fixture.store(), index, fixture, folder("album"), 2).await;

    assert!(outcome.commit.is_some(), "the deletion committed");
    assert_eq!(outcome.deleted.len(), 3);
    let (discarded, kept, refused) = reported(&outcome);
    assert_eq!(
        discarded,
        [
            &entry_path("album/c.jpg"),
            &entry_path("album/sub/a.jpg"),
            &entry_path("album/sub/b.jpg"),
        ],
        "each copy is reported discarded, in Entry Path order",
    );
    assert!(kept.is_empty() && refused.is_empty());
    assert!(outcome.unavailable.is_empty());

    let mut in_trash = fixture.fs().discarded();
    in_trash.sort();
    let mut expected = vec![a, b, c];
    expected.sort();
    assert_eq!(in_trash, expected, "the copies went to the trash");
    for path in ["album/sub/a.jpg", "album/sub/b.jpg", "album/c.jpg"] {
        assert!(
            !any_row(index, path).await,
            "the row of {path} is forgotten with its file",
        );
    }
    let root = fixture.folder();
    assert!(
        !fixture.fs().holds(&root.join("album/sub")),
        "the emptied folder is removed",
    );
    assert!(
        !fixture.fs().holds(&root.join("album")),
        "and the folder above it, which it emptied in turn",
    );
    assert!(fixture.fs().holds(root), "the mapped root is never removed");
    assert!(fixture.fs().holds(&beside), "a file nobody deleted stays");
    assert!(
        current(index, "d.jpg").await.is_some(),
        "and its Entry with it"
    );

    let after = sync(fixture, 3).await;
    assert!(after.added.is_empty(), "nothing is carried back in");
    assert!(
        after.surfaced.is_empty(),
        "and the next sync has nothing left to say of them: {:?}",
        after.surfaced,
    );
}

/// EP-15: an edited copy is kept where it is, its row with it, and reported —
/// and the folder holding it stays.
pub async fn an_edited_copy_is_kept_and_reported(fixture: &DeleteUnderTest) {
    let index = fixture.index();
    map_registered(fixture).await;
    let a = write(fixture, "album/a.jpg", b"the first file");
    let b = write(fixture, "album/b.jpg", b"the second file");
    sync(fixture, 1).await;
    fixture.fs().write_file(&b, b"what the person wrote since");
    touch(fixture.fs(), &b, NEWER);

    let outcome = delete_with_copies(fixture.store(), index, fixture, folder("album"), 2).await;

    let (discarded, kept, refused) = reported(&outcome);
    assert_eq!(discarded, [&entry_path("album/a.jpg")]);
    assert_eq!(
        kept,
        [&entry_path("album/b.jpg")],
        "the edited copy is reported as kept",
    );
    assert!(refused.is_empty());
    assert_eq!(fixture.fs().discarded(), [a]);
    assert_eq!(
        fixture.fs().content(&b).as_deref(),
        Some(&b"what the person wrote since"[..]),
        "the person's change is where they left it",
    );
    assert!(
        present(index, "album/b.jpg").await,
        "its row stays, which is what reports it again at every sync",
    );
    assert!(
        fixture.fs().holds(&fixture.folder().join("album")),
        "the folder holding it is not empty, and stays",
    );

    let after = sync(fixture, 3).await;
    assert!(
        after.added.is_empty(),
        "the kept copy is never carried back in"
    );
    assert!(
        after.surfaced.iter().any(
            |one| matches!(one, Surfaced::KeptEdited { path } if path == &entry_path("album/b.jpg"))
        ),
        "and the next sync reports it as kept too: {:?}",
        after.surfaced,
    );
}

/// EP-15: a copy already gone from the disk is nothing to discard and nothing to
/// report; its row goes, as the next sync would forget it.
pub async fn a_copy_gone_from_disk_is_left_alone(fixture: &DeleteUnderTest) {
    let index = fixture.index();
    map_registered(fixture).await;
    let a = write(fixture, "a.jpg", b"the file's bytes");
    sync(fixture, 1).await;
    fixture.fs().remove_file(&a);

    let outcome = delete_with_copies(fixture.store(), index, fixture, paths(&["a.jpg"]), 2).await;

    assert!(outcome.commit.is_some(), "the deletion committed");
    assert!(
        outcome.local.is_empty(),
        "nothing to report of a file that is not there: {:?}",
        outcome.local,
    );
    assert!(outcome.unavailable.is_empty());
    assert!(fixture.fs().discarded().is_empty());
    assert!(!any_row(index, "a.jpg").await, "the row is forgotten");
}

/// EP-13, EP-15: nothing is discarded out of a root that is not the folder its
/// mapping was recorded against. The copy and its row stay, the refusal is
/// reported with what refused it, and the next sync — under the right root —
/// discards it.
pub async fn a_root_that_does_not_vouch_for_itself_leaves_the_copy_to_the_next_sync(
    fixture: &DeleteUnderTest,
) {
    let index = fixture.index();
    map_registered(fixture).await;
    let a = write(fixture, "a.jpg", b"the file's bytes");
    sync(fixture, 1).await;
    // Another registration's marker now stands in the root.
    fixture
        .fs()
        .plant_marker(fixture.folder(), &RootMarkerId::from_bytes([0xaa; 8]));

    let outcome = delete_with_copies(fixture.store(), index, fixture, paths(&["a.jpg"]), 2).await;

    assert!(
        outcome.commit.is_some(),
        "the deletion committed all the same"
    );
    assert!(current(index, "a.jpg").await.is_none());
    assert!(
        outcome.local.iter().any(|one| matches!(
            one,
            Surfaced::DiscardRefused { path, cause }
                if path == &entry_path("a.jpg")
                    && matches!(cause.as_ref(), DescentError::Refused {
                        reason: RootRefused::MarkerMismatch,
                        ..
                    })
        )),
        "the discard is refused for the root, and said so: {:?}",
        outcome.local,
    );
    assert!(fixture.fs().discarded().is_empty());
    assert!(fixture.fs().holds(&a), "the copy stays");
    assert!(present(index, "a.jpg").await, "and its row with it");

    fixture.fs().plant_marker(fixture.folder(), &registered());
    let after = sync(fixture, 3).await;
    assert!(
        after
            .surfaced
            .iter()
            .any(|one| matches!(one, Surfaced::Discarded { path } if path == &entry_path("a.jpg"))),
        "the next sync discards it: {:?}",
        after.surfaced,
    );
    assert_eq!(fixture.fs().discarded(), [a]);
}

/// EP-12, EP-15: a mapped root that is not there is reported, and nothing
/// under it is discarded or forgotten — an unplugged disk is not a folder emptied.
pub async fn an_unavailable_root_is_reported_and_its_copies_left_alone(fixture: &DeleteUnderTest) {
    let index = fixture.index();
    let root = fixture.folder().join("disk");
    fixture.fs().create_dir(&root);
    fixture.fs().plant_marker(&root, &registered());
    index
        .set_mapping(Mapping::new(None, root.clone()).expecting(registered()))
        .await
        .expect("recording a mapping must succeed");
    crate::sync_conformance::fixtures::write(fixture.fs(), &root, "a.jpg", b"the file's bytes");
    sync(fixture, 1).await;
    fixture.fs().remove_dir_all(&root);

    let outcome = delete_with_copies(fixture.store(), index, fixture, paths(&["a.jpg"]), 2).await;

    assert!(outcome.commit.is_some(), "the deletion committed");
    assert_eq!(outcome.unavailable.len(), 1, "the root is reported");
    assert_eq!(outcome.unavailable[0].reason, RootUnavailable::Missing);
    assert!(outcome.local.is_empty(), "{:?}", outcome.local);
    assert!(fixture.fs().discarded().is_empty());
    assert!(
        present(index, "a.jpg").await,
        "the row is nobody's evidence while the root is away, and stays",
    );
}

/// CP-1, EP-15: a discard the desktop's trash refuses leaves the deletion
/// committed — the Entry is gone from the Library — and the copy and its row
/// where they are, with the refusal reported; the next sync asks again.
pub async fn a_refused_discard_leaves_the_deletion_committed_and_is_reported(
    fixture: &DeleteUnderTest,
) {
    let index = fixture.index();
    map_registered(fixture).await;
    let a = write(fixture, "a.jpg", b"the file's bytes");
    sync(fixture, 1).await;
    fixture.fs().fail_on(LocalOperation::Discarding, 1);

    let outcome = delete_with_copies(fixture.store(), index, fixture, paths(&["a.jpg"]), 2).await;

    assert!(outcome.commit.is_some(), "the deletion committed");
    assert_eq!(outcome.deleted, vec![entry_path("a.jpg")]);
    assert!(current(index, "a.jpg").await.is_none(), "and it stands");
    assert!(
        outcome.local.iter().any(|one| matches!(
            one,
            Surfaced::DiscardRefused { path, cause }
                if path == &entry_path("a.jpg")
                    && matches!(cause.as_ref(), DescentError::Io(io)
                        if matches!(io.operation, LocalOperation::Discarding))
        )),
        "the refusal is reported with what refused it: {:?}",
        outcome.local,
    );
    assert!(fixture.fs().holds(&a), "the copy is left where it was");
    assert!(present(index, "a.jpg").await, "and its row with it");

    let after = sync(fixture, 3).await;
    assert!(
        after
            .surfaced
            .iter()
            .any(|one| matches!(one, Surfaced::Discarded { path } if path == &entry_path("a.jpg"))),
        "the next sync asks again: {:?}",
        after.surfaced,
    );
    assert_eq!(fixture.fs().discarded(), [a]);
}

/// EP-15: a deletion made on another device touches nothing of this one's,
/// whatever it is handed: its copies are its own next sync's to decide.
pub async fn a_deletion_on_another_device_leaves_this_devices_copies_to_its_sync(
    fixture: &DeleteUnderTest,
) {
    map_registered(fixture).await;
    let a = write(fixture, "a.jpg", b"the file's bytes");
    sync(fixture, 1).await;

    let outcome = delete_with_copies(
        fixture.store(),
        fixture.other(),
        fixture,
        paths(&["a.jpg"]),
        2,
    )
    .await;

    assert!(outcome.commit.is_some(), "the deletion committed");
    assert!(
        outcome.local.is_empty(),
        "the deleting device holds no copy of its own: {:?}",
        outcome.local,
    );
    assert!(fixture.fs().holds(&a), "this device's copy is untouched");
    assert!(present(fixture.index(), "a.jpg").await);

    let after = sync(fixture, 3).await;
    assert!(
        after
            .surfaced
            .iter()
            .any(|one| matches!(one, Surfaced::Discarded { path } if path == &entry_path("a.jpg"))),
        "its own next sync discards it: {:?}",
        after.surfaced,
    );
    assert_eq!(fixture.fs().discarded(), [a]);
}

/// EP-15, EP-17: deleting an Entry carried in at a path an operating system
/// keeps — before the rule left such names out — leaves whatever stands at
/// that path on this device where it is: the file is the system's, nobody
/// handed it to the Library, and the scan would never have found it there. Its
/// row is forgotten, as for a copy gone from disk.
pub async fn an_entry_at_a_system_files_path_is_deleted_without_its_local_file(
    fixture: &DeleteUnderTest,
) {
    let index = fixture.index();
    map_registered(fixture).await;
    let inner = "albums/.Spotlight-V100/notes.txt";
    let file = write(fixture, inner, b"what the volume keeps there now");
    sync(fixture, 1).await;
    assert!(
        current(index, inner).await.is_some(),
        "deeper down a volume's folder name is the person's, and is carried in",
    );

    // The same folder mapped as a top-level mapping's root: the Entry now
    // stands at that root's top, where the rule leaves the name out.
    let albums = fixture.folder().join("albums");
    fixture.fs().plant_marker(&albums, &registered());
    index
        .set_mapping(Mapping::new(Some(entry_path("albums")), albums).expecting(registered()))
        .await
        .expect("recording a mapping must succeed");

    let outcome = delete_with_copies(fixture.store(), index, fixture, paths(&[inner]), 2).await;

    assert!(outcome.commit.is_some(), "the deletion committed");
    assert!(current(index, inner).await.is_none(), "the Entry is gone");
    assert!(
        outcome.local.is_empty(),
        "nothing is said of a file the deletion does not look for: {:?}",
        outcome.local,
    );
    assert!(fixture.fs().discarded().is_empty(), "nothing is discarded");
    assert_eq!(
        fixture.fs().content(&file).as_deref(),
        Some(&b"what the volume keeps there now"[..]),
        "the file at that path is left where it is",
    );
    assert!(!any_row(index, inner).await, "and the row is forgotten");
}
