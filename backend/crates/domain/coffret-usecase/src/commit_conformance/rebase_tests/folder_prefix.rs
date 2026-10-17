use coffret_model::ContainerKind;

use super::batches::{entry, rename, renaming};
use super::devices::Devices;
use crate::commit::{EntryRef, PreparedBatch};
use crate::commit_conformance::fixtures::{container_id, path, prepared};

// EP-7, rule 8: a folder rename is exactly the Entries it was prepared with,
// so an Entry another writer added under the old folder stays there; and an
// addition under a folder another writer renamed is not remapped by its
// prefix.
#[tokio::test]
async fn nothing_is_remapped_by_a_folder_prefix() {
    let renamed_folder = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    renamed_folder.upload(4).await;
    renamed_folder
        .rival(PreparedBatch::adding(vec![prepared(
            4,
            ContainerKind::OneFile,
            &["albums/n.jpg"],
        )]))
        .await;
    let outcome = renamed_folder
        .commit(
            &renamed_folder.ours,
            renaming(vec![rename(entry(1, 0), "albums/a.jpg", "photos/a.jpg")]),
        )
        .await
        .expect("the folder rename commits what it was prepared with");
    assert_eq!(outcome.record.renames().len(), 1);
    assert_eq!(
        EntryRef::renamed_by(&outcome.record.renames()[0]),
        entry(1, 0)
    );
    assert_eq!(
        renamed_folder
            .at(&renamed_folder.ours, "albums/n.jpg")
            .await
            .container_id,
        container_id(4),
        "the Entry added meanwhile stays under the old folder",
    );

    let added_under = Devices::holding(&[(1, ContainerKind::OneFile, &["albums/a.jpg"])]).await;
    added_under.upload(4).await;
    added_under
        .rival(renaming(vec![rename(
            entry(1, 0),
            "albums/a.jpg",
            "photos/a.jpg",
        )]))
        .await;
    let outcome = added_under
        .commit(
            &added_under.ours,
            PreparedBatch::adding(vec![prepared(4, ContainerKind::OneFile, &["albums/z.jpg"])]),
        )
        .await
        .expect("an addition under the renamed folder commits where it was prepared");
    assert_eq!(
        outcome.record.additions()[0].entries()[0].path,
        path("albums/z.jpg")
    );
    assert!(added_under.vacant(&added_under.ours, "photos/z.jpg").await);
}
