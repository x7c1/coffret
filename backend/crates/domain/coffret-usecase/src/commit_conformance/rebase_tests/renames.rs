use coffret_model::ContainerKind;

use super::batches::{conflict, entry, is, rename, renaming, updating};
use super::devices::Devices;
use crate::commit::{CommitError, EntryRef, PendingChange, PreparedBatch, RebaseRule};
use crate::commit_conformance::fixtures::{container_id, path, prepared};
use crate::commit_conformance::library::Library;
use crate::generations::generation;
use crate::index_error::RenameRule;

// EP-7, rule 3: a rename prepared against an Entry another writer then
// replaced is retargeted to the replacement's Entry, because the name it was
// prepared against now names that Entry.
#[tokio::test]
async fn a_rename_follows_the_entry_an_update_replaced_it_with() {
    let devices = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    devices.upload(3).await;

    devices.rival(updating(3, 1, "albums/a.jpg")).await;
    let outcome = devices
        .commit(
            &devices.ours,
            renaming(vec![rename(entry(1, 0), "albums/a.jpg", "albums/b.jpg")]),
        )
        .await
        .expect("the rename follows the update and commits");

    let renames = outcome.record.renames();
    assert_eq!(renames.len(), 1);
    assert_eq!(EntryRef::renamed_by(&renames[0]), entry(3, 0));
    assert_eq!(renames[0].path(), &path("albums/b.jpg"));
    assert!(outcome.record.additions().is_empty());
    assert!(outcome.record.removals().is_empty());
    let renamed = devices.at(&devices.ours, "albums/b.jpg").await;
    assert_eq!(renamed.container_id, container_id(3));
    assert_eq!(renamed.entry.captured_path, path("albums/a.jpg"));
}

// EP-7, rule 3, otherwise: where the record that removed the Container leaves
// nothing at the name the rename was prepared against, there is no Entry to
// carry the rename to, and it conflicts.
#[tokio::test]
async fn a_rename_of_an_entry_removed_meanwhile_conflicts() {
    let devices = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;

    devices
        .rival(PreparedBatch::default().removing(vec![container_id(1)]))
        .await;
    let (pending, record, rule, at) = conflict(
        devices
            .commit(
                &devices.ours,
                renaming(vec![rename(entry(1, 0), "albums/a.jpg", "albums/b.jpg")]),
            )
            .await,
    );

    assert!(
        is(pending, PendingChange::Rename(entry(1, 0))),
        "{pending:?}"
    );
    assert_eq!(record, generation(2));
    assert!(matches!(rule, RebaseRule::RenamedEntryReplaced), "{rule:?}");
    assert_eq!(at, "albums/a.jpg");
    assert!(
        !devices.has_head(3).await,
        "nothing was committed (spec: CP-1)"
    );
}

// EP-7, rules 2 and 3: an update and a rename of one Entry reach the same
// catalog whichever commits first — the update's Entry at the rename's path,
// captured under the name both were prepared against.
#[tokio::test]
async fn an_update_and_a_rename_converge_in_either_order() {
    let update = || updating(3, 1, "albums/a.jpg");
    let rename = || renaming(vec![rename(entry(1, 0), "albums/a.jpg", "albums/b.jpg")]);

    let rename_first = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    rename_first.upload(3).await;
    rename_first.rival(rename()).await;
    rename_first
        .commit(&rename_first.ours, update())
        .await
        .expect("the update rebases over the rename");

    let update_first = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    update_first.upload(3).await;
    update_first.rival(update()).await;
    update_first
        .commit(&update_first.ours, rename())
        .await
        .expect("the rename rebases over the update");

    let catalog = rename_first.catalog().await;
    assert_eq!(catalog, update_first.catalog().await);
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].container_id, container_id(3));
    assert_eq!(catalog[0].entry.path, path("albums/b.jpg"));
    assert_eq!(catalog[0].entry.captured_path, path("albums/a.jpg"));
}

// EP-7, rule 4: two renames of one Entry are two writes to it, and the one
// rebased is refused rather than either name winning (spec: CP-7).
#[tokio::test]
async fn a_rename_of_an_entry_renamed_meanwhile_conflicts() {
    let devices = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;

    devices
        .rival(renaming(vec![rename(
            entry(1, 0),
            "albums/a.jpg",
            "albums/c.jpg",
        )]))
        .await;
    let (pending, record, rule, at) = conflict(
        devices
            .commit(
                &devices.ours,
                renaming(vec![rename(entry(1, 0), "albums/a.jpg", "albums/b.jpg")]),
            )
            .await,
    );

    assert!(
        is(pending, PendingChange::Rename(entry(1, 0))),
        "{pending:?}"
    );
    assert_eq!(record, generation(2));
    assert!(matches!(rule, RebaseRule::RenamedEntryRenamed), "{rule:?}");
    assert_eq!(at, "albums/a.jpg");
    assert!(
        !devices.has_head(3).await,
        "nothing was committed (spec: CP-1)"
    );
    assert_eq!(
        devices.at(&devices.ours, "albums/c.jpg").await.container_id,
        container_id(1),
        "the rival's name stands",
    );
}

// EP-7, rule 5: a destination another writer filled since — by an addition or
// by a rename — and still holds is a conflict.
#[tokio::test]
async fn a_rename_onto_a_destination_filled_meanwhile_conflicts() {
    let by_addition = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    by_addition.upload(4).await;
    by_addition
        .rival(PreparedBatch::adding(vec![prepared(
            4,
            ContainerKind::OneFile,
            &["albums/b.jpg"],
        )]))
        .await;

    let by_rename = Devices::holding(&[
        (1, ContainerKind::OneFile, &["albums/a.jpg"]),
        (2, ContainerKind::OneFile, &["albums/x.jpg"]),
    ])
    .await;
    by_rename
        .rival(renaming(vec![rename(
            entry(2, 0),
            "albums/x.jpg",
            "albums/b.jpg",
        )]))
        .await;

    for (filled, devices) in [("an addition", &by_addition), ("a rename", &by_rename)] {
        let (pending, record, rule, at) = conflict(
            devices
                .commit(
                    &devices.ours,
                    renaming(vec![rename(entry(1, 0), "albums/a.jpg", "albums/b.jpg")]),
                )
                .await,
        );
        assert!(
            is(pending, PendingChange::Rename(entry(1, 0))),
            "{filled}: {pending:?}"
        );
        assert_eq!(record, generation(2), "{filled}");
        assert!(
            matches!(rule, RebaseRule::DestinationTaken),
            "{filled}: {rule:?}"
        );
        assert_eq!(at, "albums/b.jpg", "{filled}");
        assert!(
            !devices.has_head(3).await,
            "{filled}: nothing was committed"
        );
    }
}

// FM-15, CP-1: a rename the head does not admit — here onto a path an Entry
// already held when the batch was prepared, so no rebase rule is in question —
// is refused as the batch's own, naming the element and the rule, before the
// commit writes anything to Storage.
#[tokio::test]
async fn a_rename_the_head_does_not_admit_is_refused_before_anything_is_written() {
    let devices = Devices::holding(&[
        (1, ContainerKind::OneFile, &["albums/a.jpg"]),
        (2, ContainerKind::OneFile, &["albums/b.jpg"]),
    ])
    .await;
    let names = Library::read(&devices.store).await.names();
    let puts = devices.store.puts();

    let refused = devices
        .commit(
            &devices.ours,
            renaming(vec![rename(entry(1, 0), "albums/a.jpg", "albums/b.jpg")]),
        )
        .await;

    assert!(
        matches!(
            refused,
            Err(CommitError::RenameRefused {
                element: 0,
                rule: RenameRule::PathTaken,
            })
        ),
        "{refused:?}"
    );
    assert_eq!(devices.store.puts(), puts, "nothing was uploaded");
    assert_eq!(
        Library::read(&devices.store).await.names(),
        names,
        "Storage holds what it held before the commit"
    );
    assert_eq!(
        devices.at(&devices.ours, "albums/a.jpg").await.container_id,
        container_id(1),
        "the catalog is as it was",
    );
}
