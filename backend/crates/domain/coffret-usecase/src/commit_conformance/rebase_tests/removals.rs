use coffret_model::ContainerKind;

use super::batches::{entry, rebuilding, rename, renaming};
use super::devices::Devices;
use crate::commit::PreparedBatch;
use crate::commit_conformance::fixtures::{container_id, path};

// EP-7, rule 7: a removal names its Container, so an Entry another writer
// renamed since still leaves with it — removed outright, or left out of a
// rebuilt Pack — and nothing else of the batch changes.
#[tokio::test]
async fn a_removal_still_removes_an_entry_renamed_meanwhile() {
    let outright = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    outright
        .rival(renaming(vec![rename(
            entry(1, 0),
            "albums/a.jpg",
            "albums/b.jpg",
        )]))
        .await;
    let outcome = outright
        .commit(
            &outright.ours,
            PreparedBatch::default().removing(vec![container_id(1)]),
        )
        .await
        .expect("the removal still applies");
    assert_eq!(outcome.record.removals(), [container_id(1)]);
    assert!(outcome.record.additions().is_empty());
    assert!(outcome.record.renames().is_empty());
    assert!(
        outright.catalog().await.is_empty(),
        "the renamed Entry left too"
    );

    let rebuilt =
        Devices::holding(&[(1, ContainerKind::Pack, &["albums/a.jpg", "albums/b.jpg"])]).await;
    rebuilt.upload(3).await;
    rebuilt
        .rival(renaming(vec![rename(
            entry(1, 0),
            "albums/a.jpg",
            "albums/x.jpg",
        )]))
        .await;
    let outcome = rebuilt
        .commit(&rebuilt.ours, rebuilding(3, 1, &[("albums/b.jpg", 1)]))
        .await
        .expect("the rebuild that leaves the renamed Entry out still applies");
    let added = &outcome.record.additions()[0];
    assert_eq!(added.entries().len(), 1);
    assert_eq!(added.entries()[0].path, path("albums/b.jpg"));
    assert_eq!(added.entries()[0].captured_path, path("albums/b.jpg"));
    let catalog = rebuilt.catalog().await;
    assert_eq!(catalog.len(), 1, "the renamed Entry left with its Pack");
    assert_eq!(catalog[0].entry.path, path("albums/b.jpg"));
}
