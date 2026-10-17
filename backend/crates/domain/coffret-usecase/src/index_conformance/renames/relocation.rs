use coffret_model::ContainerKind;

use super::{relocate_to, rename, renaming};
use crate::index::Index;
use crate::index_conformance::fixtures::{addition, container_id, observation, path, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// The file this device has of a renamed Entry stays where it was placed, and
/// its row with it, marked to follow the Entry: it is no longer reported as
/// present at its own path nor as left behind by the Library. Renames chain,
/// swap, and undo themselves the way the Entries do, and a file placed afresh
/// at the path follows nothing (spec: FM-15, EP-10).
pub async fn the_file_of_a_renamed_entry_waits_to_follow_it(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"],
            )],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    for text in ["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"] {
        index
            .mark_present(observation(text, 100))
            .await
            .expect("recording a materialized file must succeed");
    }

    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![
                rename(1, 0, "albums/b.jpg"),
                rename(1, 1, "albums/a.jpg"),
                rename(1, 2, "albums/x.jpg"),
            ],
        ))
        .await
        .expect("applying the renames must succeed");
    assert_eq!(
        relocate_to(index, "albums/a.jpg").await.as_deref(),
        Some("albums/b.jpg")
    );
    assert_eq!(
        relocate_to(index, "albums/b.jpg").await.as_deref(),
        Some("albums/a.jpg")
    );
    assert_eq!(
        relocate_to(index, "albums/c.jpg").await.as_deref(),
        Some("albums/x.jpg")
    );
    assert!(
        index
            .present_under(None)
            .await
            .expect("reading what this device has must succeed")
            .is_empty(),
        "no file is a copy of the Entry at its own path any more"
    );
    assert!(
        index
            .present_without_entry()
            .await
            .expect("reading what the Library left behind must succeed")
            .is_empty(),
        "and none was left behind: every one of their Entries is still current"
    );

    // A second rename of an Entry carries its file's mark along, and one back
    // to where the file stands clears it.
    index
        .apply(renaming(
            record(2, vec![], vec![]),
            vec![
                rename(1, 0, "albums/a.jpg"),
                rename(1, 1, "albums/b.jpg"),
                rename(1, 2, "albums/y.jpg"),
            ],
        ))
        .await
        .expect("applying the second renames must succeed");
    assert_eq!(relocate_to(index, "albums/a.jpg").await, None);
    assert_eq!(relocate_to(index, "albums/b.jpg").await, None);
    assert_eq!(
        relocate_to(index, "albums/c.jpg").await.as_deref(),
        Some("albums/y.jpg")
    );

    // A file placed afresh is the Entry at its path, and follows nothing.
    index
        .mark_present(observation("albums/c.jpg", 200))
        .await
        .expect("recording a materialized file must succeed");
    assert_eq!(relocate_to(index, "albums/c.jpg").await, None);
}

/// A file waiting to follow an Entry a later record removes follows nothing
/// any more, and is left behind like the file of any removed Entry — even
/// where another Entry of that record is renamed onto the path it waited for,
/// whose own file then waits for it there.
/// One whose Entry is replaced at that path by a Container the same record
/// adds keeps waiting for it, as a file at its own path keeps its row through
/// a replacement (spec: EP-6, EP-10, FM-15).
pub async fn the_file_of_a_removed_entry_stops_waiting_for_it(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![
                addition(1, ContainerKind::OneFile, &["albums/a.jpg"]),
                addition(2, ContainerKind::OneFile, &["albums/b.jpg"]),
                addition(3, ContainerKind::OneFile, &["inbox/c.jpg"]),
            ],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    for text in ["albums/a.jpg", "albums/b.jpg", "inbox/c.jpg"] {
        index
            .mark_present(observation(text, 100))
            .await
            .expect("recording a materialized file must succeed");
    }
    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "2024/a.jpg"), rename(2, 0, "2024/b.jpg")],
        ))
        .await
        .expect("applying the renames must succeed");

    index
        .apply(renaming(
            record(
                2,
                vec![addition(4, ContainerKind::Pack, &["2024/b.jpg"])],
                vec![container_id(1), container_id(2)],
            ),
            vec![rename(3, 0, "2024/a.jpg")],
        ))
        .await
        .expect("applying the removals must succeed");

    assert_eq!(
        relocate_to(index, "albums/a.jpg").await,
        None,
        "the Entry now at 2024/a.jpg is not the one the file is a copy of"
    );
    assert_eq!(
        relocate_to(index, "albums/b.jpg").await.as_deref(),
        Some("2024/b.jpg"),
        "a replacement at the path carries the wait over"
    );
    assert_eq!(
        relocate_to(index, "inbox/c.jpg").await.as_deref(),
        Some("2024/a.jpg"),
        "the file of the Entry renamed onto the vacated path waits for it there"
    );
    let left_behind: Vec<String> = index
        .present_without_entry()
        .await
        .expect("reading what the Library left behind must succeed")
        .into_iter()
        .map(|local| local.observation.path.as_str().to_owned())
        .collect();
    assert_eq!(left_behind, ["albums/a.jpg"]);
}

/// A device that adopts a Snapshot past renames of Entries it has files of
/// marks those files to follow them, as replaying the records in between
/// would have, rather than taking them for files the Library left behind
/// (spec: CK-9, EP-10, FM-15).
pub async fn a_restore_past_a_rename_marks_the_file_to_follow_it(fixture: &IndexUnderTest) {
    let records = [
        record(
            0,
            vec![
                addition(1, ContainerKind::OneFile, &["albums/a.jpg"]),
                addition(2, ContainerKind::OneFile, &["albums/b.jpg"]),
                addition(3, ContainerKind::OneFile, &["albums/c.jpg"]),
            ],
            vec![],
        ),
        renaming(record(1, vec![], vec![]), vec![rename(2, 0, "x/b.jpg")]),
        renaming(
            record(2, vec![], vec![]),
            vec![rename(1, 0, "2024/a.jpg"), rename(2, 0, "y/b.jpg")],
        ),
        record(3, vec![], vec![container_id(3)]),
    ];
    let index = fixture.index();
    let other = fixture.other();
    for record in &records {
        other
            .apply(record.clone())
            .await
            .expect("replaying a record must succeed");
    }
    index
        .apply(records[0].clone())
        .await
        .expect("applying the first record must succeed");
    for text in ["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"] {
        index
            .mark_present(observation(text, 100))
            .await
            .expect("recording a materialized file must succeed");
    }
    index
        .apply(records[1].clone())
        .await
        .expect("applying the first rename must succeed");

    index
        .restore(
            other
                .snapshot()
                .await
                .expect("a replayed catalog has a state to checkpoint"),
        )
        .await
        .expect("adopting a later Snapshot must succeed");

    assert_eq!(
        relocate_to(index, "albums/a.jpg").await.as_deref(),
        Some("2024/a.jpg")
    );
    assert_eq!(
        relocate_to(index, "albums/b.jpg").await.as_deref(),
        Some("y/b.jpg"),
        "a file already waiting follows its Entry on"
    );
    assert_eq!(relocate_to(index, "albums/c.jpg").await, None);
    let left_behind: Vec<String> = index
        .present_without_entry()
        .await
        .expect("reading what the Library left behind must succeed")
        .into_iter()
        .map(|local| local.observation.path.as_str().to_owned())
        .collect();
    assert_eq!(
        left_behind,
        ["albums/c.jpg"],
        "only the file of the removed Entry is left behind"
    );
}

/// The file of a removed Entry that stands where a rename put another Entry
/// is not that Entry's copy: the device forgets its row rather than ending
/// the wait into a record that would claim the other Entry present, so the
/// file is neither reported nor selected for `update`, and a fetch of that
/// Entry there is refused rather than overwriting it. The other Entry's own
/// file keeps waiting for it (spec: EP-10, EP-11, FM-15).
pub async fn the_file_of_a_removed_entry_is_forgotten_under_a_renamed_one(
    fixture: &IndexUnderTest,
) {
    let records = swap_then_remove();
    let index = fixture.index();
    index
        .apply(records[0].clone())
        .await
        .expect("applying the first record must succeed");
    mark_both_present(index).await;
    for record in &records[1..] {
        index
            .apply(record.clone())
            .await
            .expect("applying a later record must succeed");
    }

    assert_forgotten_under_the_other(index).await;
}

/// Adopting a Snapshot past the removal of an Entry whose file waits to
/// follow it ends the wait the way the replay does: where the path the file
/// stands at holds another Entry, the row is forgotten (spec: CK-9, EP-10,
/// FM-15).
pub async fn a_restore_past_the_removal_forgets_the_file_under_a_renamed_entry(
    fixture: &IndexUnderTest,
) {
    let records = swap_then_remove();
    let index = fixture.index();
    let other = fixture.other();
    for record in &records {
        other
            .apply(record.clone())
            .await
            .expect("replaying a record must succeed");
    }
    index
        .apply(records[0].clone())
        .await
        .expect("applying the first record must succeed");
    mark_both_present(index).await;
    index
        .apply(records[1].clone())
        .await
        .expect("applying the swap must succeed");

    index
        .restore(
            other
                .snapshot()
                .await
                .expect("a replayed catalog has a state to checkpoint"),
        )
        .await
        .expect("adopting a later Snapshot must succeed");

    assert_forgotten_under_the_other(index).await;
}

/// Two Entries at `albums/a.jpg` and `albums/b.jpg` that swap their names,
/// and then the removal of the first one's Container.
fn swap_then_remove() -> [coffret_model::JournalRecord; 3] {
    [
        record(
            0,
            vec![
                addition(1, ContainerKind::OneFile, &["albums/a.jpg"]),
                addition(2, ContainerKind::OneFile, &["albums/b.jpg"]),
            ],
            vec![],
        ),
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(2, 0, "albums/a.jpg")],
        ),
        record(2, vec![], vec![container_id(1)]),
    ]
}

async fn mark_both_present(index: &dyn Index) {
    for text in ["albums/a.jpg", "albums/b.jpg"] {
        index
            .mark_present(observation(text, 100))
            .await
            .expect("recording a materialized file must succeed");
    }
}

/// The file of the removed Entry, at `albums/a.jpg` where the other Entry now
/// stands, has no row; the other Entry's file still waits for it there.
async fn assert_forgotten_under_the_other(index: &dyn Index) {
    assert!(
        index
            .local_entry_at(&path("albums/a.jpg"))
            .await
            .expect("reading a local file's row must succeed")
            .is_none(),
        "the file at albums/a.jpg is not a copy of the Entry standing there"
    );
    assert_eq!(
        relocate_to(index, "albums/b.jpg").await.as_deref(),
        Some("albums/a.jpg")
    );
    assert!(
        index
            .present_under(None)
            .await
            .expect("reading what this device has must succeed")
            .is_empty(),
        "no file stands for the Entry at albums/a.jpg"
    );
    assert!(
        index
            .present_without_entry()
            .await
            .expect("reading what the Library left behind must succeed")
            .is_empty(),
        "and none is reported as left behind"
    );
}
