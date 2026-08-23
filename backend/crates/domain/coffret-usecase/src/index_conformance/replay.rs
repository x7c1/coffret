use coffret_model::ContainerKind;

use crate::index_conformance::fixtures::{
    addition, checkpoint, container_id, library_state, path, record, snapshot, snapshot_name,
};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_error::IndexError;

/// A catalog that has never been given a committed state stands at none.
///
/// A fresh Index has nothing to checkpoint, and saying so is what lets a device
/// deciding where to start from — its own Index, or the newest checkpoint on
/// Storage — see that the answer is Storage (spec: CK-9).
pub async fn a_fresh_index_stands_at_no_committed_state(fixture: &IndexUnderTest) {
    let index = fixture.index();

    assert!(
        index
            .checkpoint()
            .await
            .expect("reading a fresh checkpoint must succeed")
            .is_none(),
        "a catalog that has restored and applied nothing stands at no state"
    );

    let result = index.snapshot().await;
    assert!(
        matches!(result, Err(IndexError::NoCheckpoint)),
        "expected nothing to checkpoint, got {result:?}"
    );
}

/// What a restore takes in is what a checkpoint hands back.
///
/// The Snapshot is the Library-wide content in full, so adopting one and then
/// being asked for one again yields the same value — that is what makes a
/// Snapshot written by any device a starting point for any other (spec: CK-7,
/// CK-8, RV-1).
pub async fn a_restore_round_trips_through_a_checkpoint(fixture: &IndexUnderTest) {
    let index = fixture.index();
    let adopted = snapshot(
        4,
        vec![
            addition(
                1,
                ContainerKind::Pack,
                &["albums/a.jpg", "books/page-001.png"],
            ),
            addition(2, ContainerKind::OneFile, &["notes.txt"]),
        ],
        Some(snapshot_name(4)),
    );

    index
        .restore(adopted.clone())
        .await
        .expect("restoring a Snapshot must succeed");

    assert_eq!(
        index
            .snapshot()
            .await
            .expect("a restored catalog has a state to checkpoint"),
        adopted
    );
}

/// A restore replaces the catalog rather than merging into it.
///
/// A Snapshot is the whole Library at one committed state, so what it does not
/// mention is not current — an Entry left behind from an earlier state would be
/// a path the Library no longer holds, answered as if it did (spec: RV-1).
pub async fn a_restore_replaces_the_whole_catalog(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .restore(snapshot(
            4,
            vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
            Some(snapshot_name(4)),
        ))
        .await
        .expect("restoring a Snapshot must succeed");

    let later = snapshot(
        9,
        vec![addition(2, ContainerKind::OneFile, &["notes.txt"])],
        Some(snapshot_name(9)),
    );
    index
        .restore(later.clone())
        .await
        .expect("restoring a later Snapshot must succeed");

    assert_eq!(
        index
            .snapshot()
            .await
            .expect("a restored catalog has a state to checkpoint"),
        later
    );
    assert!(
        index
            .entry_at(&path("albums/a.jpg"))
            .await
            .expect("looking a path up must succeed")
            .is_none(),
        "a path the newer Snapshot does not carry is not current"
    );
}

/// Replaying records reaches the state a restore of the head would.
///
/// This is the whole of catching up: a device starts from the newer of its own
/// Index and the newest checkpoint, then replays only the records after that
/// point, and lands where a device that took the later Snapshot lands
/// (spec: CK-9, RV-1, RV-5). Neither path opens a Container, because a record
/// carries what the Containers it adds hold (spec: CP-11).
pub async fn a_replay_reaches_what_a_restore_of_the_head_would(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .restore(snapshot(
            4,
            vec![addition(
                1,
                ContainerKind::Pack,
                &["albums/a.jpg", "albums/b.jpg"],
            )],
            Some(snapshot_name(4)),
        ))
        .await
        .expect("restoring a Snapshot must succeed");

    index
        .apply(record(
            5,
            vec![addition(2, ContainerKind::OneFile, &["notes.txt"])],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");

    // A Pack replaced by one holding only `a.jpg` is exactly how the deletion
    // of `b.jpg` is expressed (spec: PK-9, PK-10).
    index
        .apply(record(
            6,
            vec![addition(3, ContainerKind::Pack, &["albums/a.jpg"])],
            vec![container_id(1)],
        ))
        .await
        .expect("replaying a replacement must succeed");

    let head = snapshot(
        6,
        vec![
            addition(2, ContainerKind::OneFile, &["notes.txt"]),
            addition(3, ContainerKind::Pack, &["albums/a.jpg"]),
        ],
        Some(snapshot_name(6)),
    );
    fixture
        .other()
        .restore(head.clone())
        .await
        .expect("restoring the head's Snapshot must succeed");

    let replayed = index
        .snapshot()
        .await
        .expect("a caught-up catalog has a state to checkpoint");
    let restored = fixture
        .other()
        .snapshot()
        .await
        .expect("a restored catalog has a state to checkpoint");

    // Where each catalog started is its own provenance and is the one thing the
    // two do not share at a common head: replaying carries a catalog past the
    // Snapshot it adopted without moving that record forward (spec: CK-9).
    assert_eq!(
        replayed.adopted_from,
        Some(snapshot_name(4)),
        "a replay leaves the Snapshot this catalog started from as it was"
    );
    assert_eq!(
        restored.adopted_from,
        Some(snapshot_name(6)),
        "a restore records the Snapshot it adopted"
    );

    assert_eq!(library_state(replayed), library_state(restored));
}

/// Removing a Container takes its Entries with it.
///
/// The Entries of a removed Container are not separately removed by the record:
/// removal is expressed at the Container, and a catalog that kept the Entries
/// would answer with a location inside an object that is no longer current
/// (spec: CP-1, CP-14).
pub async fn removing_a_container_removes_the_entries_it_held(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![
                addition(
                    1,
                    ContainerKind::Pack,
                    &["albums/a.jpg", "books/page-001.png"],
                ),
                addition(2, ContainerKind::OneFile, &["notes.txt"]),
            ],
            vec![],
        ))
        .await
        .expect("replaying the first record must succeed");

    index
        .apply(record(1, vec![], vec![container_id(1)]))
        .await
        .expect("replaying a removal must succeed");

    let content = index
        .snapshot()
        .await
        .expect("an applied catalog has a state to checkpoint");
    let paths: Vec<&str> = content
        .entries
        .iter()
        .map(|entry| entry.path().as_str())
        .collect();
    assert_eq!(paths, ["notes.txt"]);

    let containers: Vec<_> = content
        .containers
        .iter()
        .map(|container| container.id)
        .collect();
    assert_eq!(containers, [container_id(2)]);
    assert_eq!(content.checkpoint, checkpoint(1));
    assert_eq!(
        content.adopted_from, None,
        "a catalog that has only replayed records has adopted no Snapshot"
    );
}
