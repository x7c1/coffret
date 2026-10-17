use coffret_model::{ContainerKind, JournalRecord};

use super::{names, relocate_to, rename, renaming};
use crate::index::Index;
use crate::index_conformance::fixtures::{addition, container_id, observation, record};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_error::IndexError;

/// A record below the head the catalog stands at is refused as already
/// applied, and nothing of the catalog or of this device's rows changes
/// (spec: CK-9, RV-5).
///
/// All but the first are records a second application would not refuse on
/// its own: the swap would swap the two Entries back, the rename would carry
/// an Entry a later record renamed again back to where it was, the removal
/// would take a current Container out, and the files waiting to follow the
/// swapped Entries would be marked for paths those Entries no longer stand at.
/// So the refusal is by generation, before anything else of the record is
/// looked at.
pub async fn a_record_below_the_checkpoint_is_refused_and_changes_nothing(
    fixture: &IndexUnderTest,
) {
    let index = fixture.index();
    let [first, swap, rename, removal] = past_a_swap(index).await;
    let stood_at = index
        .snapshot()
        .await
        .expect("an applied catalog has a state to checkpoint");
    let named = names(index).await;
    let waited_for = waiting(index).await;

    for (again, generation) in [(first, 0), (swap, 1), (rename, 2), (removal, 2)] {
        let result = index.apply(again).await;
        assert!(
            matches!(
                &result,
                Err(IndexError::AlreadyApplied { generation: refused, checkpoint })
                    if refused.get() == generation && checkpoint.get() == 3
            ),
            "expected the record at {generation} to be refused as already applied, \
             got {result:?}"
        );
    }

    assert_eq!(
        index
            .snapshot()
            .await
            .expect("an applied catalog has a state to checkpoint"),
        stood_at,
        "the refused records leave the Containers, the Entries and the checkpoint \
         as they were",
    );
    assert_eq!(names(index).await, named);
    assert_eq!(
        waiting(index).await,
        waited_for,
        "and every file still waits for where its Entry stands now"
    );
}

/// The Library [`a_record_below_the_checkpoint_is_refused_and_changes_nothing`]
/// replays, applied to `index` up to its head at 3, as four records below it.
///
/// Two Entries swap their names at 1 and nothing later touches them; a third
/// is renamed at 2 and renamed again at 3, which also removes a fourth
/// Container. The removal-only record at 2 is one the Library never
/// committed, which nothing but its generation refuses.
async fn past_a_swap(index: &dyn Index) -> [JournalRecord; 4] {
    let first = record(
        0,
        vec![
            addition(1, ContainerKind::OneFile, &["albums/a.jpg"]),
            addition(2, ContainerKind::OneFile, &["albums/b.jpg"]),
            addition(3, ContainerKind::OneFile, &["inbox/c.jpg"]),
            addition(4, ContainerKind::OneFile, &["inbox/d.jpg"]),
        ],
        vec![],
    );
    let swap = renaming(
        record(1, vec![], vec![]),
        vec![rename(1, 0, "albums/b.jpg"), rename(2, 0, "albums/a.jpg")],
    );
    let rename_c = renaming(record(2, vec![], vec![]), vec![rename(3, 0, "2024/c.jpg")]);
    let last = renaming(
        record(3, vec![], vec![container_id(4)]),
        vec![rename(3, 0, "2024/z.jpg")],
    );

    index
        .apply(first.clone())
        .await
        .expect("applying the first record must succeed");
    for text in ["albums/a.jpg", "albums/b.jpg"] {
        index
            .mark_present(observation(text, 100))
            .await
            .expect("recording a materialized file must succeed");
    }
    for later in [swap.clone(), rename_c.clone(), last] {
        index
            .apply(later)
            .await
            .expect("applying each later record must succeed");
    }
    let removal = record(2, vec![], vec![container_id(2)]);
    [first, swap, rename_c, removal]
}

/// Where the two materialized files wait to follow their Entries to.
async fn waiting(index: &dyn Index) -> [Option<String>; 2] {
    [
        relocate_to(index, "albums/a.jpg").await,
        relocate_to(index, "albums/b.jpg").await,
    ]
}
