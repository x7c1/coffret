//! What a Snapshot's content refuses to be built out of.

use super::*;
use crate::error::Error;
use crate::generation::Generation;
use crate::testing::{container_summary, entry_location, keyring_commitment, master_key_epoch};

fn checkpoint() -> IndexCheckpoint {
    IndexCheckpoint::new(
        master_key_epoch(),
        Generation::new(4),
        Generation::new(4),
        None,
        keyring_commitment(),
    )
    .expect("a fixture holds a checkpoint of one head")
}

// FM-16, EP-3: the Containers are in ID order and the Entries in Entry Path
// order, both strictly — a repeat names one Container twice, or holds two
// current Entries at one path, which EP-5 rules out.
#[test]
fn a_snapshot_out_of_canonical_order_cannot_exist() {
    let reversed = SnapshotContent::new(
        checkpoint(),
        None,
        vec![container_summary(2), container_summary(1)],
        Vec::new(),
    );
    assert!(
        matches!(
            reversed,
            Err(Error::CollectionOutOfCanonicalOrder {
                collection: "containers",
                index: 1,
            })
        ),
        "expected reversed Containers to be refused, got {reversed:?}",
    );

    let repeated = SnapshotContent::new(
        checkpoint(),
        None,
        vec![container_summary(1)],
        vec![
            entry_location(1, "albums/one.jpg", 0, 4),
            entry_location(1, "albums/one.jpg", 4, 4),
        ],
    );
    assert!(
        matches!(
            repeated,
            Err(Error::CollectionOutOfCanonicalOrder {
                collection: "entries",
                index: 1,
            })
        ),
        "expected a repeated Entry Path to be refused, got {repeated:?}",
    );
}

// FM-16: an Entry names its Container among the ones the Snapshot lists, or
// a device restoring from it would hold an Entry in a Container it does not
// have.
#[test]
fn a_snapshot_whose_entry_names_no_container_cannot_exist() {
    let result = SnapshotContent::new(
        checkpoint(),
        None,
        vec![container_summary(1)],
        vec![
            entry_location(1, "albums/one.jpg", 0, 4),
            entry_location(9, "albums/two.jpg", 0, 4),
        ],
    );

    assert!(
        matches!(
            result,
            Err(Error::SnapshotEntryWithoutContainer {
                entry: 1,
                container_id,
            }) if container_id == crate::testing::container_id(9)
        ),
        "expected the dangling Entry to be refused with its Container, got {result:?}",
    );
}

// The one thing `canonical` adds is the sort, and the one thing it does not
// add is tolerance: an input a sort cannot repair is refused exactly as
// `new` refuses it.
#[test]
fn canonical_sorts_and_then_holds_to_the_same_rule() {
    let containers = vec![container_summary(2), container_summary(1)];
    let entries = vec![
        entry_location(2, "albums/two.jpg", 0, 4),
        entry_location(1, "albums/one.jpg", 0, 4),
    ];
    let sorted =
        SnapshotContent::canonical(checkpoint(), None, containers.clone(), entries.clone())
            .expect("an unsorted but valid content sorts into one");
    let expected = SnapshotContent::new(
        checkpoint(),
        None,
        vec![container_summary(1), container_summary(2)],
        vec![
            entry_location(1, "albums/one.jpg", 0, 4),
            entry_location(2, "albums/two.jpg", 0, 4),
        ],
    )
    .expect("the same content, handed over sorted");
    assert_eq!(sorted, expected);

    let duplicated = SnapshotContent::canonical(
        checkpoint(),
        None,
        vec![
            container_summary(2),
            container_summary(1),
            container_summary(2),
        ],
        Vec::new(),
    );
    assert!(
        matches!(
            duplicated,
            Err(Error::CollectionOutOfCanonicalOrder {
                collection: "containers",
                ..
            })
        ),
        "sorting cannot make a duplicate disappear, got {duplicated:?}",
    );
}
