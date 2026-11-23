use coffret_model::ContainerKind;

use crate::index_conformance::fixtures::{addition, container_id, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// The count of current Entries is the number the whole set holds: none on a
/// fresh catalog, every Entry a record adds, and none of a Container a record
/// removes (spec: CK-9).
pub async fn the_current_entries_are_counted(fixture: &IndexUnderTest) {
    let index = fixture.index();
    let counted = || async {
        index
            .entry_count()
            .await
            .expect("counting the current Entries must succeed")
    };
    assert_eq!(counted().await, 0, "a fresh catalog holds no Entry");

    index
        .apply(record(
            0,
            vec![
                addition(1, ContainerKind::Pack, &["albums/a.jpg", "albums/b.jpg"]),
                addition(2, ContainerKind::OneFile, &["notes.txt"]),
            ],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");
    assert_eq!(counted().await, 3);

    index
        .apply(record(1, vec![], vec![container_id(1)]))
        .await
        .expect("replaying a removal must succeed");
    let all = index
        .entries_under(None)
        .await
        .expect("reading the Library must succeed");
    assert_eq!(counted().await, all.len() as u64);
    assert_eq!(
        counted().await,
        1,
        "the removed Pack's Entries left with it"
    );
}
