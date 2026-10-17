use coffret_model::ContainerKind;

use super::batches::{entry, observed, rebuilding, rename, renaming, superseding};
use super::devices::Devices;
use crate::commit::{EntryRef, PreparedBatch};
use crate::commit_conformance::fixtures::{container_id, path};
use crate::index::Index;

// EP-7, rule 6: a deletion's rebuilt Pack, prepared before another writer
// renamed an Entry it keeps, is committed as it was uploaded — no second
// upload — and its record carries the kept Entry at the renamed path.
#[tokio::test]
async fn a_rebuild_keeps_its_pack_and_carries_a_renamed_entry_under_its_new_path() {
    let devices =
        Devices::holding(&[(1, ContainerKind::Pack, &["albums/a.jpg", "albums/b.jpg"])]).await;
    devices.upload(3).await;
    let rebuilt = container_id(3).object_name();

    devices
        .rival(renaming(vec![rename(
            entry(1, 1),
            "albums/b.jpg",
            "albums/c.jpg",
        )]))
        .await;
    let outcome = devices
        .commit(&devices.ours, rebuilding(3, 1, &[("albums/b.jpg", 1)]))
        .await
        .expect("the rebuild rebases over the rename and commits");

    let record = &outcome.record;
    assert_eq!(record.removals(), [container_id(1)]);
    let added = &record.additions()[0];
    assert_eq!(
        added.container().id,
        container_id(3),
        "the Pack already uploaded"
    );
    assert_eq!(added.entries()[0].path, path("albums/c.jpg"));
    assert_eq!(added.entries()[0].captured_path, path("albums/b.jpg"));
    assert_eq!(
        devices.store.puts_of(&rebuilt),
        1,
        "the rebuilt Pack went up once, before the commit, and the rebase sent it again never",
    );
    assert!(devices.vacant(&devices.ours, "albums/a.jpg").await);
    assert!(devices.vacant(&devices.ours, "albums/b.jpg").await);
    assert_eq!(
        devices.at(&devices.ours, "albums/c.jpg").await.container_id,
        container_id(3)
    );
}

// EP-7, rule 6, for a freeze: a Pack absorbing one-file Containers, prepared
// before another writer renamed one of the absorbed Entries, takes that Entry
// over at its new path, captured under the name it was absorbed under. The
// file it was drawn from is not recorded as the Entry at its own path; it
// waits to follow the Entry (spec: PK-7, EP-10).
#[tokio::test]
async fn a_pack_takes_over_an_absorbed_entry_at_the_path_a_rename_gave_it() {
    let devices = Devices::holding(&[
        (1, ContainerKind::OneFile, &["albums/a.jpg"]),
        (2, ContainerKind::OneFile, &["albums/b.jpg"]),
    ])
    .await;
    for (text, seed) in [("albums/a.jpg", 1), ("albums/b.jpg", 2)] {
        devices
            .ours
            .mark_present(observed(text, entry(seed, 0)))
            .await
            .expect("recording the placed file must succeed");
    }
    devices.upload(3).await;
    let pack = PreparedBatch::adding(vec![superseding(
        3,
        ContainerKind::Pack,
        &["albums/a.jpg", "albums/b.jpg"],
        &[(0, entry(1, 0)), (1, entry(2, 0))],
    )])
    .removing(vec![container_id(1), container_id(2)])
    .materializing(vec![
        observed("albums/a.jpg", entry(3, 0)),
        observed("albums/b.jpg", entry(3, 1)),
    ]);

    devices
        .rival(renaming(vec![rename(
            entry(2, 0),
            "albums/b.jpg",
            "albums/c.jpg",
        )]))
        .await;
    let outcome = devices
        .commit(&devices.ours, pack)
        .await
        .expect("the Pack rebases over the rename and commits");

    let added = &outcome.record.additions()[0];
    assert_eq!(added.container().id, container_id(3));
    assert_eq!(added.entries()[0].path, path("albums/a.jpg"));
    assert_eq!(added.entries()[1].path, path("albums/c.jpg"));
    assert_eq!(added.entries()[1].captured_path, path("albums/b.jpg"));
    assert!(devices.vacant(&devices.ours, "albums/b.jpg").await);
    let file = devices
        .ours
        .local_entry_at(&path("albums/b.jpg"))
        .await
        .expect("reading the device's row must succeed")
        .expect("the file's row is kept");
    assert_eq!(file.relocate_to, Some(path("albums/c.jpg")));
}

// EP-7, rules 6 and 3: a rebuild and a rename of an Entry it keeps reach the
// same catalog whichever commits first.
#[tokio::test]
async fn a_rebuild_and_a_rename_of_a_kept_entry_converge_in_either_order() {
    let files: &[(u8, ContainerKind, &[&str])] =
        &[(1, ContainerKind::Pack, &["albums/a.jpg", "albums/b.jpg"])];
    let rebuild = || rebuilding(3, 1, &[("albums/b.jpg", 1)]);
    let rename = || renaming(vec![rename(entry(1, 1), "albums/b.jpg", "albums/c.jpg")]);

    let rename_first = Devices::holding(files).await;
    rename_first.upload(3).await;
    rename_first.rival(rename()).await;
    rename_first
        .commit(&rename_first.ours, rebuild())
        .await
        .expect("the rebuild rebases over the rename");

    let rebuild_first = Devices::holding(files).await;
    rebuild_first.upload(3).await;
    rebuild_first.rival(rebuild()).await;
    let outcome = rebuild_first
        .commit(&rebuild_first.ours, rename())
        .await
        .expect("the rename rebases over the rebuild");
    assert_eq!(
        EntryRef::renamed_by(&outcome.record.renames()[0]),
        entry(3, 0),
        "the rename was retargeted to the rebuilt Pack's Entry",
    );

    let catalog = rename_first.catalog().await;
    assert_eq!(catalog, rebuild_first.catalog().await);
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].container_id, container_id(3));
    assert_eq!(catalog[0].entry.path, path("albums/c.jpg"));
    assert_eq!(catalog[0].entry.captured_path, path("albums/b.jpg"));
}
