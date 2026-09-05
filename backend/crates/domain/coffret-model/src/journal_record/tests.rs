//! What a record refuses to be built out of, and what one answers once built.

use super::*;
use crate::error::{Error, Result};
use crate::testing::{
    container_id, container_summary, generation, keyring_commitment, master_key_epoch, table,
};

/// The addition of the Container `seed` names, holding one Entry.
fn addition(seed: u8) -> ContainerAddition {
    ContainerAddition::new(container_summary(seed), table(&[(0, 16)]))
        .expect("a fixture holds a table that tiles")
}

/// The record at `generation` succeeding `prev`, adding and removing what
/// the two lists name.
fn record(
    number: u64,
    prev: Option<u64>,
    additions: Vec<ContainerAddition>,
    removals: Vec<ContainerId>,
) -> Result<JournalRecord> {
    JournalRecord::new(
        generation(number),
        prev.map(generation),
        master_key_epoch(),
        keyring_commitment(),
        None,
        None,
        additions,
        removals,
    )
}

// FM-15: a record at generation g succeeds head g − 1, and the first record
// succeeds nothing. Anything else is a chain no commit wrote.
#[test]
fn a_journal_record_names_its_predecessor_or_cannot_exist() {
    record(0, None, vec![addition(1)], Vec::new())
        .expect("the Library's first head succeeds nothing");
    record(5, Some(4), vec![addition(1)], Vec::new())
        .expect("every later head succeeds the one before it");

    for (generation, prev) in [(0, Some(0)), (5, None), (5, Some(3)), (5, Some(5))] {
        let result = record(generation, prev, Vec::new(), Vec::new());
        assert!(
            matches!(
                result,
                Err(Error::JournalRecordPredecessorMismatch {
                    generation: refused,
                    prev: claimed,
                }) if refused.get() == generation
                    && claimed.map(Generation::get) == prev
            ),
            "expected {generation}/{prev:?} to be refused with both, got {result:?}",
        );
    }
}

// FM-15: both collections are in Container ID order and strictly so — a
// repeat names one Container twice within one record.
#[test]
fn a_journal_record_out_of_canonical_order_cannot_exist() {
    let reversed = record(1, Some(0), vec![addition(2), addition(1)], Vec::new());
    assert!(
        matches!(
            reversed,
            Err(Error::CollectionOutOfCanonicalOrder {
                collection: "additions",
                index: 1,
            })
        ),
        "expected reversed additions to be refused, got {reversed:?}",
    );

    let repeated = record(
        1,
        Some(0),
        Vec::new(),
        vec![container_id(3), container_id(3)],
    );
    assert!(
        matches!(
            repeated,
            Err(Error::CollectionOutOfCanonicalOrder {
                collection: "removals",
                index: 1,
            })
        ),
        "expected a repeated removal to be refused, got {repeated:?}",
    );
}

// The writers gather their additions in spool order and their removals in
// the order the Containers they displace turned up, so `canonical` is what
// they build through — and it holds to `new`'s rule once the sort is done.
#[test]
fn canonical_sorts_a_record_and_then_holds_to_the_same_rule() {
    let sorted = JournalRecord::canonical(
        generation(1),
        Some(generation(0)),
        master_key_epoch(),
        keyring_commitment(),
        None,
        None,
        vec![addition(2), addition(1)],
        vec![container_id(9), container_id(4)],
    )
    .expect("an unsorted but valid record sorts into one");

    let expected = record(
        1,
        Some(0),
        vec![addition(1), addition(2)],
        vec![container_id(4), container_id(9)],
    )
    .expect("the same record, handed over sorted");
    assert_eq!(sorted, expected);

    let duplicated = JournalRecord::canonical(
        generation(1),
        Some(generation(0)),
        master_key_epoch(),
        keyring_commitment(),
        None,
        None,
        Vec::new(),
        vec![container_id(9), container_id(4), container_id(9)],
    );
    assert!(
        matches!(
            duplicated,
            Err(Error::CollectionOutOfCanonicalOrder {
                collection: "removals",
                ..
            })
        ),
        "sorting cannot make a duplicate disappear, got {duplicated:?}",
    );
}

// CK-1: the checkpoint a record reaches is the one construction where the
// two generations coincide, so it cannot fail and does not pretend to.
#[test]
fn the_checkpoint_a_record_reaches_stands_at_its_own_generation() {
    let record =
        record(5, Some(4), vec![addition(1)], Vec::new()).expect("a record of one addition");
    let checkpoint = record.checkpoint();

    assert_eq!(checkpoint.head_generation(), generation(5));
    assert_eq!(checkpoint.journal_generation(), generation(5));
}
