use coffret_model::{ContainerAddition, ContainerKind, DerivedFrom, JournalRecord};

use super::{rename, renaming};
use crate::index::Index;
use crate::index_conformance::device_state::{assert_device_state_intact, seed_device_state};
use crate::index_conformance::fixtures::{addition, container_id, path, record};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_error::{IndexError, RenameRule};

/// The catalog every refusal below is tried against: an ordinary file this
/// device has on disk, a Pack of two, and a derived Entry, with the seeded
/// device state beside them.
async fn standing(index: &dyn Index) {
    seed_device_state(index).await;
    let mut derived = addition(4, ContainerKind::OneFile, &[".coffret-derived/thumb/a.jpg"]);
    let (container, mut entries) = derived.into_parts();
    entries[0].derived_from = Some(DerivedFrom {
        container_id: container_id(1),
        path: path("albums/a.jpg"),
    });
    derived = ContainerAddition::new(container, entries)
        .expect("marking an Entry derived leaves its table one that tiles");
    index
        .apply(record(
            0,
            vec![
                addition(1, ContainerKind::OneFile, &["albums/a.jpg"]),
                addition(2, ContainerKind::Pack, &["pack/x.jpg", "pack/y.jpg"]),
                derived,
            ],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
}

/// Applies `record` to the [`standing`] catalog, expects `element` of its
/// renames to be refused for a rule `broke` accepts, and the catalog and the
/// device's own rows to be exactly as they were (spec: CP-1, FM-15).
///
/// The rule is matched rather than compared, so a field a refusal gains later
/// does not turn every case here into a different assertion.
async fn refused_whole(
    fixture: &IndexUnderTest,
    record: JournalRecord,
    element: usize,
    broke: impl Fn(&RenameRule) -> bool,
) {
    let index = fixture.index();
    standing(index).await;
    let before = index
        .snapshot()
        .await
        .expect("an applied catalog has a state to checkpoint");

    let result = index.apply(record).await;
    assert!(
        matches!(
            &result,
            Err(IndexError::RenameRefused { element: found, rule })
                if *found == element && broke(rule)
        ),
        "expected rename {element} to be refused for the rule this case names, got {result:?}"
    );
    assert_eq!(
        index
            .snapshot()
            .await
            .expect("the catalog still stands where it did"),
        before,
        "a refused record leaves the catalog exactly as it was"
    );
    assert_device_state_intact(index).await;
}

/// A rename of an Entry in a Container that is not current is refused.
pub async fn a_rename_in_a_container_that_is_not_current_is_refused(fixture: &IndexUnderTest) {
    refused_whole(
        fixture,
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(9, 0, "albums/z.jpg")],
        ),
        1,
        |rule| {
            matches!(rule, RenameRule::ContainerNotCurrent { container_id: found }
                if *found == container_id(9))
        },
    )
    .await;
}

/// A rename of an Entry whose Container the same record removes is refused.
pub async fn a_rename_in_a_container_the_record_removes_is_refused(fixture: &IndexUnderTest) {
    refused_whole(
        fixture,
        renaming(
            record(1, vec![], vec![container_id(1)]),
            vec![rename(1, 0, "albums/b.jpg")],
        ),
        0,
        |rule| {
            matches!(rule, RenameRule::ContainerRemoved { container_id: found }
                if *found == container_id(1))
        },
    )
    .await;
}

/// A rename naming a number past its Container's entry table is refused
/// (spec: FM-9).
pub async fn a_rename_past_the_entry_table_is_refused(fixture: &IndexUnderTest) {
    refused_whole(
        fixture,
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(2, 2, "pack/z.jpg")],
        ),
        1,
        |rule| {
            matches!(rule, RenameRule::EntryNumberOutsideTable { container_id: found, entry_number: 2 }
                if *found == container_id(2))
        },
    )
    .await;
}

/// A rename to the path the Entry stands at already is refused.
pub async fn a_rename_that_changes_nothing_is_refused(fixture: &IndexUnderTest) {
    refused_whole(
        fixture,
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/a.jpg")],
        ),
        0,
        |rule| matches!(rule, RenameRule::PathUnchanged),
    )
    .await;
}

/// A rename of a derived Entry is refused: this version renames none.
pub async fn a_rename_of_a_derived_entry_is_refused(fixture: &IndexUnderTest) {
    refused_whole(
        fixture,
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(4, 0, "thumbs/a.jpg")],
        ),
        1,
        |rule| matches!(rule, RenameRule::DerivedEntry),
    )
    .await;
}

/// A rename onto an Entry the record leaves where it is is refused
/// (spec: EP-5).
pub async fn a_rename_onto_a_standing_entry_is_refused(fixture: &IndexUnderTest) {
    refused_whole(
        fixture,
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(2, 0, "pack/y.jpg")],
        ),
        1,
        |rule| matches!(rule, RenameRule::PathTaken),
    )
    .await;
}

/// Two renames onto one path are refused, the later of the two named
/// (spec: EP-5).
pub async fn two_renames_onto_one_path_are_refused(fixture: &IndexUnderTest) {
    refused_whole(
        fixture,
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/z.jpg"), rename(2, 1, "albums/z.jpg")],
        ),
        1,
        |rule| matches!(rule, RenameRule::PathClaimedTwice { earlier: 0 }),
    )
    .await;
}

/// A rename onto the path the same record adds an Entry at is refused
/// (spec: EP-5, FM-15).
pub async fn a_rename_onto_the_records_own_addition_is_refused(fixture: &IndexUnderTest) {
    refused_whole(
        fixture,
        renaming(
            record(
                1,
                vec![addition(5, ContainerKind::OneFile, &["albums/n.jpg"])],
                vec![],
            ),
            vec![rename(1, 0, "albums/n.jpg")],
        ),
        0,
        |rule| {
            matches!(rule, RenameRule::PathAdded { container_id: found }
                if *found == container_id(5))
        },
    )
    .await;
}
