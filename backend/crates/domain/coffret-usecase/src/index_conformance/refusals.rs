use coffret_model::{ContainerKind, EntryLocation};

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
pub async fn two_entries_at_one_path_are_refused(fixture: &IndexUnderTest) {
    let index = fixture.index();

    let mut claimed_twice = addition(1, ContainerKind::Pack, &["albums/a.jpg"]);
    let duplicate = claimed_twice.entries[0].clone();
    claimed_twice.entries.push(duplicate);

    let result = index.apply(record(0, vec![claimed_twice], vec![])).await;
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

/// One Container added twice in one record is refused.
pub async fn one_container_added_twice_is_refused(fixture: &IndexUnderTest) {
    let index = fixture.index();

    let result = index
        .apply(record(
            0,
            vec![
                addition(1, ContainerKind::Pack, &["albums/a.jpg"]),
                addition(1, ContainerKind::Pack, &["albums/b.jpg"]),
            ],
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

/// An Entry whose Container is not in the content is refused.
///
/// A record carries the Entries of the Containers it adds, and a Snapshot
/// carries every current Entry with its Container (spec: CP-11, CK-7). An Entry
/// pointing at neither would be a path the catalog answers with a location
/// inside nothing.
pub async fn an_entry_without_its_container_is_refused(fixture: &IndexUnderTest) {
    let index = fixture.index();

    let mut content = snapshot(
        4,
        vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
        Some(snapshot_name(4)),
    );
    content.entries.push(EntryLocation {
        container_id: container_id(9),
        entry: addition(9, ContainerKind::OneFile, &["notes.txt"])
            .entries
            .remove(0),
    });

    let result = index.restore(content).await;
    assert!(
        matches!(
            &result,
            Err(IndexError::UnknownContainer { container_id: id }) if *id == container_id(9)
        ),
        "expected an Entry to name a Container the content holds, got {result:?}"
    );

    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "a refused restore leaves the catalog where it was"
    );
}

/// A refused operation leaves a catalog that already holds a state untouched.
///
/// The refusals above are all raised part-way through the work, once some of a
/// record's Containers or some of a Snapshot's Entries have been taken in. What
/// the port promises is that none of it is kept: a replay is the all-or-nothing
/// a commit is (spec: CP-1), and an implementation that stopped where it failed
/// would leave a catalog holding Containers no committed state ever held, which
/// is a Snapshot no other device could reach. Empty catalogs cannot show this,
/// so this case refuses against one that already stands at a committed state
/// and carries device state besides.
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

    // A Snapshot refused after the catalog it replaces has been cleared.
    let mut claimed_twice = snapshot(
        9,
        vec![addition(3, ContainerKind::OneFile, &["notes.txt"])],
        Some(snapshot_name(9)),
    );
    let duplicate = claimed_twice.entries[0].clone();
    claimed_twice.entries.push(duplicate);

    let result = index.restore(claimed_twice).await;
    assert!(
        matches!(&result, Err(IndexError::DuplicatePath { path }) if path.as_str() == "notes.txt"),
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
