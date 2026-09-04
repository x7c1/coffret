use coffret_model::ContainerKind;

use crate::index_conformance::device_state::{assert_device_state_intact, seed_device_state};
use crate::index_conformance::fixtures::{addition, container_id, record, snapshot, snapshot_name};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_error::IndexError;

/// Two Entries at one Entry Path are refused, and nothing of the record lands.
///
/// At every committed Library state one Entry Path identifies at most one
/// current Entry (spec: EP-5), and the commit is where that is enforced
/// (spec: EP-6). A record reaching a catalog with two is describing a state no
/// commit could have produced, so it is reported rather than half-applied —
/// which is also the shape of the atomicity a replay owes: all of a record or
/// none of it (spec: CP-1).
///
/// The two Entries are in two Containers of one record, which is where EP-6
/// leaves the possibility: one Container's own entry table cannot name a path
/// twice, because that is a rule
/// [`ContainerAddition`](coffret_model::ContainerAddition) holds.
pub async fn two_entries_at_one_path_are_refused(fixture: &IndexUnderTest) {
    let index = fixture.index();

    let claimed_twice = vec![
        addition(1, ContainerKind::Pack, &["albums/a.jpg"]),
        addition(2, ContainerKind::Pack, &["albums/a.jpg"]),
    ];

    let result = index.apply(record(0, claimed_twice, vec![])).await;
    assert!(
        matches!(&result, Err(IndexError::DuplicatePath { path }) if path.as_str() == "albums/a.jpg"),
        "expected one Entry Path to admit one Entry, got {result:?}"
    );

    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "a refused record leaves the catalog where it was"
    );
}

/// One Container entering the current set twice is refused.
///
/// One record cannot name a Container twice — that is a rule
/// [`JournalRecord`](coffret_model::JournalRecord) holds (spec: FM-15) — so
/// what reaches a catalog is a later record adding a Container it already
/// holds. The Entry Paths differ, so the collision reported is the Container's
/// and not a path's.
pub async fn one_container_added_twice_is_refused(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");

    let result = index
        .apply(record(
            1,
            vec![addition(1, ContainerKind::Pack, &["albums/b.jpg"])],
            vec![],
        ))
        .await;

    assert!(
        matches!(
            &result,
            Err(IndexError::DuplicateContainer { container_id: id }) if *id == container_id(1)
        ),
        "expected a Container to be added once, got {result:?}"
    );
}

/// A refused operation leaves a catalog that already holds a state untouched.
///
/// The refusals above are raised part-way through the work, once some of a
/// record's Containers have been taken in. What the port promises is that none
/// of it is kept: a replay is the all-or-nothing a commit is (spec: CP-1), and
/// an implementation that stopped where it failed would leave a catalog holding
/// Containers no committed state ever held, which is a Snapshot no other device
/// could reach. Empty catalogs cannot show this, so this case refuses against
/// one that already stands at a committed state and carries device state
/// besides.
///
/// Only a record is refused here. A Snapshot's content holds together by the
/// time it is a [`SnapshotContent`](coffret_model::SnapshotContent) at all, so
/// there is no longer a restore an implementation could take half of.
pub async fn a_refused_operation_leaves_the_whole_catalog_as_it_was(fixture: &IndexUnderTest) {
    let index = fixture.index();
    seed_device_state(index).await;

    index
        .restore(snapshot(
            4,
            vec![addition(
                1,
                ContainerKind::Pack,
                &["albums/a.jpg", "books/page-001.png"],
            )],
            Some(snapshot_name(4)),
        ))
        .await
        .expect("restoring a Snapshot must succeed");
    let before = index
        .snapshot()
        .await
        .expect("a restored catalog has a state to checkpoint");

    // A record whose Container is new but whose Entry Path is already held: the
    // Container is taken in before the path is found to be claimed (spec: EP-5).
    let result = index
        .apply(record(
            5,
            vec![addition(2, ContainerKind::Pack, &["albums/a.jpg"])],
            vec![],
        ))
        .await;
    assert!(
        matches!(&result, Err(IndexError::DuplicatePath { path }) if path.as_str() == "albums/a.jpg"),
        "expected one Entry Path to admit one Entry, got {result:?}"
    );

    assert_eq!(
        index
            .snapshot()
            .await
            .expect("a restored catalog has a state to checkpoint"),
        before,
        "a refused operation leaves the Containers, the Entries, the checkpoint, \
         and the Snapshot this catalog was adopted from as they were"
    );
    assert_device_state_intact(index).await;
}

/// A record the catalog already holds is refused, and the catalog is left
/// standing at it (spec: CP-1, EP-5).
///
/// This is the pair of facts a device catching up leans on when it is not the
/// only one replaying into this catalog — a server answering a browser while a
/// `sync` runs in a terminal is two processes over one Index file, each having
/// listed the Journal from its own reading of the checkpoint. The refusal is
/// what keeps a record from being taken in twice, and the checkpoint standing
/// past it is the only thing that tells the loser its refusal is somebody else's
/// replay rather than a Library state no commit could have produced.
///
/// Which of the two collisions is reported is left to the implementation: a
/// record is taken in as its Containers and then their Entries, so either the
/// Container in the current set or the path already holding its Entry is the
/// first thing met, and both say the same thing about the record.
pub async fn a_record_already_applied_is_refused_and_the_checkpoint_stands(
    fixture: &IndexUnderTest,
) {
    let index = fixture.index();
    let once = record(
        0,
        vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
        vec![],
    );

    index
        .apply(once.clone())
        .await
        .expect("replaying a record must succeed");
    let stood_at = index
        .snapshot()
        .await
        .expect("an applied catalog has a state to checkpoint");

    let result = index.apply(once).await;
    assert!(
        matches!(
            &result,
            Err(IndexError::DuplicateContainer { .. } | IndexError::DuplicatePath { .. })
        ),
        "expected a record already in the catalog to collide, got {result:?}"
    );

    assert_eq!(
        index
            .snapshot()
            .await
            .expect("an applied catalog has a state to checkpoint"),
        stood_at,
        "the refused replay leaves the catalog standing at the record it holds, \
         which is what a second replayer reads to see the refusal explained",
    );
}
