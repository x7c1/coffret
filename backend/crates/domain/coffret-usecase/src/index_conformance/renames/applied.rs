use coffret_model::{ContainerKind, EntryLocation, EntryMetadata};

use super::{at, entry_number, holds, names, rename, renaming};
use crate::index_conformance::fixtures::{
    addition, checkpoint, container_id, library_state, path, record, written_as,
};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// One file renamed: it stands at its new path and nowhere else, as the same
/// Entry of the same Container, and the name its Container captured it under
/// stays what it was (spec: FM-9, FM-15).
pub async fn a_rename_changes_one_files_current_path(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![addition(1, ContainerKind::OneFile, &["inbox/a.jpg"])],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    let before = at(index, "inbox/a.jpg").await;

    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/a.jpg")],
        ))
        .await
        .expect("applying a rename must succeed");

    assert!(
        !holds(index, "inbox/a.jpg").await,
        "the old path is vacated"
    );
    let after = at(index, "albums/a.jpg").await;
    assert_eq!(after.container_id, before.container_id);
    assert_eq!(after.entry_number, before.entry_number);
    assert_eq!(after.entry.captured_path, path("inbox/a.jpg"));
    assert_eq!(
        EntryLocation {
            entry: EntryMetadata {
                path: path("inbox/a.jpg"),
                ..after.entry.clone()
            },
            ..after
        },
        before,
        "nothing but the current path changed"
    );
    assert_eq!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed"),
        Some(checkpoint(1)),
        "the checkpoint advanced to the renaming head"
    );
}

/// One Entry of a Pack renamed: the others stay where they are, and the Pack
/// is still the Container all of them live in (spec: FM-9, PK-8).
pub async fn a_rename_inside_a_pack_leaves_its_neighbours(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![addition(
                2,
                ContainerKind::Pack,
                &["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"],
            )],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");

    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![rename(2, 1, "albums/z.jpg")],
        ))
        .await
        .expect("applying a rename must succeed");

    assert_eq!(
        names(index).await,
        [
            ("albums/a.jpg".to_owned(), "albums/a.jpg".to_owned()),
            ("albums/c.jpg".to_owned(), "albums/c.jpg".to_owned()),
            ("albums/z.jpg".to_owned(), "albums/b.jpg".to_owned()),
        ]
    );
    let renamed = at(index, "albums/z.jpg").await;
    assert_eq!(
        (renamed.container_id, renamed.entry_number),
        (container_id(2), entry_number(1))
    );
    assert_eq!(
        index
            .containers_under(Some(&path("albums")))
            .await
            .expect("listing a folder's Containers must succeed")
            .iter()
            .map(|container| container.id)
            .collect::<Vec<_>>(),
        [container_id(2)],
    );
}

/// A folder's worth of renames in one record: every Entry under the old
/// folder lands under the new one, and the old folder holds nothing
/// (spec: FM-15).
pub async fn a_folders_worth_of_renames_lands_in_one_record(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![
                addition(1, ContainerKind::Pack, &["trip/a.jpg", "trip/b.jpg"]),
                addition(2, ContainerKind::OneFile, &["trip/c.mov"]),
                addition(3, ContainerKind::OneFile, &["other/d.jpg"]),
            ],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");

    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![
                rename(1, 0, "2024/trip/a.jpg"),
                rename(1, 1, "2024/trip/b.jpg"),
                rename(2, 0, "2024/trip/c.mov"),
            ],
        ))
        .await
        .expect("applying a folder's renames must succeed");

    assert!(index
        .entries_under(Some(&path("trip")))
        .await
        .expect("listing a folder must succeed")
        .is_empty());
    assert_eq!(
        names(index).await,
        [
            ("2024/trip/a.jpg".to_owned(), "trip/a.jpg".to_owned()),
            ("2024/trip/b.jpg".to_owned(), "trip/b.jpg".to_owned()),
            ("2024/trip/c.mov".to_owned(), "trip/c.mov".to_owned()),
            ("other/d.jpg".to_owned(), "other/d.jpg".to_owned()),
        ]
    );
}

/// Two Entries swapping their names is one valid record: the renames apply
/// at once, so each takes the path the other leaves (spec: EP-5, FM-15).
pub async fn two_entries_swap_their_names_in_one_record(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &["albums/a.jpg", "albums/b.jpg"],
            )],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");

    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(1, 1, "albums/a.jpg")],
        ))
        .await
        .expect("a swap is one valid record");

    assert_eq!(
        at(index, "albums/a.jpg").await.entry_number,
        entry_number(1)
    );
    assert_eq!(
        at(index, "albums/b.jpg").await.entry_number,
        entry_number(0)
    );
    assert_eq!(
        names(index).await,
        [
            ("albums/a.jpg".to_owned(), "albums/b.jpg".to_owned()),
            ("albums/b.jpg".to_owned(), "albums/a.jpg".to_owned()),
        ]
    );
}

/// The captured path stands apart from the current one exactly where a rename
/// leaves the two different: renaming away from it sets them apart, and
/// renaming back to it makes them one again, which is the catalog's whole
/// statement that the record and the Snapshot carry no `captured_path` there
/// (spec: FM-15, FM-16).
pub async fn the_captured_path_stands_apart_exactly_where_a_rename_leaves_it(
    fixture: &IndexUnderTest,
) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![
                addition(1, ContainerKind::OneFile, &["inbox/a.jpg"]),
                written_as(
                    addition(2, ContainerKind::OneFile, &["albums/b.jpg"]),
                    &[("albums/b.jpg", "inbox/b.jpg")],
                ),
            ],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");

    // Away from the captured path, and back to it.
    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/a.jpg"), rename(2, 0, "inbox/b.jpg")],
        ))
        .await
        .expect("applying the renames must succeed");

    let away = at(index, "albums/a.jpg").await;
    assert_ne!(away.entry.captured_path, away.entry.path);
    assert_eq!(away.entry.captured_path, path("inbox/a.jpg"));
    let back = at(index, "inbox/b.jpg").await;
    assert_eq!(
        back.entry.captured_path, back.entry.path,
        "renamed back to the name its Container recorded, the two are one again"
    );

    // And a checkpoint of it, adopted elsewhere, says the same.
    let other = fixture.other();
    other
        .restore(
            index
                .snapshot()
                .await
                .expect("an applied catalog has a state to checkpoint"),
        )
        .await
        .expect("restoring a Snapshot must succeed");
    assert_eq!(names(other).await, names(index).await);
}

/// A device replaying every record and a device adopting a checkpoint taken
/// between renames and replaying the rest reach one catalog, current and
/// captured paths alike (spec: CK-9, RV-5).
pub async fn a_replay_with_renames_reaches_what_a_restore_and_the_rest_would(
    fixture: &IndexUnderTest,
) {
    let records = [
        record(
            0,
            vec![
                addition(1, ContainerKind::Pack, &["trip/a.jpg", "trip/b.jpg"]),
                addition(2, ContainerKind::OneFile, &["inbox/c.jpg"]),
            ],
            vec![],
        ),
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "trip/b.jpg"), rename(1, 1, "trip/a.jpg")],
        ),
        renaming(
            record(
                2,
                vec![addition(3, ContainerKind::OneFile, &["inbox/c.jpg"])],
                vec![],
            ),
            vec![rename(2, 0, "albums/c.jpg")],
        ),
        renaming(
            record(3, vec![], vec![container_id(3)]),
            vec![rename(1, 0, "2024/a.jpg"), rename(2, 0, "inbox/c.jpg")],
        ),
    ];

    let index = fixture.index();
    let other = fixture.other();
    let (to_checkpoint, rest) = records.split_at(2);
    for record in to_checkpoint {
        index
            .apply(record.clone())
            .await
            .expect("replaying a record must succeed");
    }
    other
        .restore(
            index
                .snapshot()
                .await
                .expect("an applied catalog has a state to checkpoint"),
        )
        .await
        .expect("restoring a Snapshot must succeed");
    for record in rest {
        index
            .apply(record.clone())
            .await
            .expect("replaying a record must succeed");
        other
            .apply(record.clone())
            .await
            .expect("replaying a record after a restore must succeed");
    }

    assert_eq!(
        library_state(index.snapshot().await.expect("the replay stands somewhere")),
        library_state(
            other
                .snapshot()
                .await
                .expect("the restore stands somewhere")
        ),
    );
    assert_eq!(
        names(index).await,
        [
            ("2024/a.jpg".to_owned(), "trip/a.jpg".to_owned()),
            ("inbox/c.jpg".to_owned(), "inbox/c.jpg".to_owned()),
            ("trip/a.jpg".to_owned(), "trip/b.jpg".to_owned()),
        ]
    );
}

/// A Container's Entries are listed in entry number order and at the paths
/// the catalog holds them at now, whatever order a rename leaves those paths
/// in — which is how a reader holding the Container's own table finds where
/// each of its Entries stands (spec: FM-9, FM-15). A Container that is not
/// current holds none.
pub async fn a_containers_entries_are_listed_by_number_at_their_current_paths(
    fixture: &IndexUnderTest,
) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![
                addition(1, ContainerKind::OneFile, &["inbox/z.jpg"]),
                addition(
                    2,
                    ContainerKind::Pack,
                    &["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"],
                ),
            ],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![rename(2, 0, "trip/a.jpg")],
        ))
        .await
        .expect("applying a rename must succeed");

    let listed = |entries: Vec<EntryLocation>| {
        entries
            .into_iter()
            .map(|location| {
                (
                    location.container_id,
                    location.entry_number,
                    location.entry.path.as_str().to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    let in_pack = index
        .entries_in(container_id(2))
        .await
        .expect("listing a Container's Entries must succeed");
    assert_eq!(
        listed(in_pack),
        [
            (container_id(2), entry_number(0), "trip/a.jpg".to_owned()),
            (container_id(2), entry_number(1), "albums/b.jpg".to_owned()),
            (container_id(2), entry_number(2), "albums/c.jpg".to_owned()),
        ],
    );
    let in_one_file = index
        .entries_in(container_id(1))
        .await
        .expect("listing a Container's Entries must succeed");
    assert_eq!(
        listed(in_one_file),
        [(container_id(1), entry_number(0), "inbox/z.jpg".to_owned())],
    );
    assert!(
        index
            .entries_in(container_id(9))
            .await
            .expect("listing a Container's Entries must succeed")
            .is_empty(),
        "a Container the catalog does not hold holds no Entry",
    );
}
