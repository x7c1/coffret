use coffret_model::ContainerKind;

use super::batches::{conflict, entry, is, observed, rename, renaming, updating};
use super::devices::{Devices, BASE};
use crate::commit::{PendingChange, PreparedBatch, RebaseRule};
use crate::commit_conformance::fixtures::{container_id, path, prepared};
use crate::commit_conformance::racing_store::RacingStore;
use crate::generations::generation;
use crate::index::Index;

// EP-7, rule 1: a rename that gave another Entry the path an addition wants is
// a conflict, as an addition at that path already is. The addition is
// refused, naming itself, the rival record and the path, and nothing of it is
// committed.
#[tokio::test]
async fn an_addition_meeting_a_rename_onto_its_path_conflicts() {
    let devices = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    devices.upload(3).await;
    let addition =
        PreparedBatch::adding(vec![prepared(3, ContainerKind::OneFile, &["albums/b.jpg"])]);

    devices
        .rival(renaming(vec![rename(
            entry(1, 0),
            "albums/a.jpg",
            "albums/b.jpg",
        )]))
        .await;
    let (pending, record, rule, at) = conflict(devices.commit(&devices.ours, addition).await);

    assert!(
        is(pending, PendingChange::Addition(entry(3, 0))),
        "{pending:?}"
    );
    assert_eq!(record, generation(2));
    assert!(matches!(rule, RebaseRule::RenamedOntoAddedPath), "{rule:?}");
    assert_eq!(at, "albums/b.jpg");
    assert!(
        !devices.has_head(3).await,
        "nothing was committed (spec: CP-1)"
    );
    assert_eq!(
        devices.at(&devices.ours, "albums/b.jpg").await.container_id,
        container_id(1),
        "the renamed Entry keeps the path",
    );
}

// EP-7, rule 2: an update prepared against an Entry another writer renamed
// follows the rename. It replaces the same Entry at its new path; the new
// Container's captured path stays the name the update was prepared under, and
// the old name is not brought back.
#[tokio::test]
async fn an_update_follows_a_rename_of_its_entry() {
    let devices = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    devices
        .ours
        .mark_present(observed("albums/a.jpg"))
        .await
        .expect("recording the placed file must succeed");
    devices.upload(3).await;

    devices
        .rival(renaming(vec![rename(
            entry(1, 0),
            "albums/a.jpg",
            "albums/b.jpg",
        )]))
        .await;
    let outcome = devices
        .commit(&devices.ours, updating(3, 1, "albums/a.jpg"))
        .await
        .expect("the update follows the rename and commits");

    let record = &outcome.record;
    assert_eq!(record.generation(), generation(3));
    assert_eq!(record.removals(), [container_id(1)]);
    let added = &record.additions()[0];
    assert_eq!(added.container().id, container_id(3));
    assert_eq!(added.entries()[0].path, path("albums/b.jpg"));
    assert_eq!(added.entries()[0].captured_path, path("albums/a.jpg"));

    let updated = devices.at(&devices.ours, "albums/b.jpg").await;
    assert_eq!(updated.container_id, container_id(3));
    assert_eq!(updated.entry.captured_path, path("albums/a.jpg"));
    assert!(devices.vacant(&devices.ours, "albums/a.jpg").await);
    // The file the update was drawn from stays where it is and waits to
    // follow the Entry, rather than being recorded as the Entry at its own
    // path (spec: EP-10).
    let file = devices
        .ours
        .local_entry_at(&path("albums/a.jpg"))
        .await
        .expect("reading the device's row must succeed")
        .expect("the file's row is kept");
    assert_eq!(file.relocate_to, Some(path("albums/b.jpg")));
}

// EP-7, rule 2, after losing the slot rather than before the first attempt:
// the rival lands at the moment this writer creates its record, and the
// attempt that rebases follows the rename the same way (spec: CP-4).
#[tokio::test]
async fn an_update_that_loses_the_slot_to_a_rename_follows_it() {
    let devices = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    devices.upload(3).await;
    let racing = RacingStore::letting_in(
        &devices.store,
        &devices.theirs,
        &devices.keys,
        renaming(vec![rename(entry(1, 0), "albums/a.jpg", "albums/b.jpg")])
            .prepared_at(Some(generation(BASE))),
    );

    let outcome = devices
        .commit_through(&racing, &devices.ours, updating(3, 1, "albums/a.jpg"))
        .await
        .expect("the update rebases over the rename and commits");

    assert_eq!(outcome.attempts, 2, "the first attempt lost the slot");
    assert_eq!(outcome.record.generation(), generation(3));
    assert_eq!(
        outcome.record.additions()[0].entries()[0].path,
        path("albums/b.jpg")
    );
}
