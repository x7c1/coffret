//! What a length claimed for a control object may cost before it is believed.
//!
//! Storage is outside the trust boundary, so the size of an object — what a
//! provider reports for it, or what a name leads to — is an adversary's number
//! until the tag over it verifies. These cases are about the moment before that:
//! a ceiling per kind, held against the claim, so that reading a control object
//! never costs more than a control object of that kind can be.

use coffret_model::{ControlObjectKind, ControlObjectName, Generation};

use super::ceiling::{
    check_control_object_len, max_control_object_len, max_control_object_len_at,
    MAX_INDEX_SNAPSHOT_LEN, MAX_JOURNAL_RECORD_LEN, MAX_KEYRING_LEN,
};
use super::testing::{encode_with, name, ALL_KINDS, GENERATION, SET_DIGEST};
use crate::error::Error;

// A length past a kind's ceiling is refused, and the refusal names the kind, the
// length claimed, and what that kind may be. No object is needed: the whole
// point is that the claim is answered before anything is read for it.
#[test]
fn a_length_past_a_kinds_ceiling_is_refused() {
    for kind in ALL_KINDS {
        let limit = max_control_object_len(kind);
        let result = check_control_object_len(kind, limit + 1);
        assert!(
            matches!(
                result,
                Err(Error::ControlObjectTooLong {
                    kind: refused,
                    len,
                    limit: stated,
                }) if refused == kind && len == limit + 1 && stated == limit
            ),
            "expected {kind:?} to refuse a length of {}, got {result:?}",
            limit + 1
        );
        assert!(
            check_control_object_len(kind, limit).is_ok(),
            "the ceiling itself is a length {kind:?} may be",
        );
    }
}

// A reader has the name and not yet the kind — the kind rides in the header,
// inside the answer — so the bound it reads with is the largest of the kinds
// FM-12's table admits at that name. Anything smaller would refuse a legitimate
// object of the other admitted kind.
#[test]
fn a_name_is_bounded_by_the_largest_kind_it_admits() {
    for kind in ALL_KINDS {
        let name = name(kind);
        assert!(
            max_control_object_len_at(&name) >= max_control_object_len(kind),
            "{name} admits {kind:?} and must take in an object of that kind",
        );
    }

    let generation = Generation::new(GENERATION);
    // The head chain admits a Journal record and an activation Snapshot, and a
    // Snapshot is the larger of the two.
    assert_eq!(
        max_control_object_len_at(&ControlObjectName::head(generation)),
        MAX_INDEX_SNAPSHOT_LEN,
    );
    assert_eq!(
        max_control_object_len_at(&ControlObjectName::index_snapshot(generation)),
        MAX_INDEX_SNAPSHOT_LEN,
    );
    assert_eq!(
        max_control_object_len_at(
            &ControlObjectName::keyring_replica(
                generation,
                SET_DIGEST,
                coffret_model::ReplicaPosition::new(1, 3).expect("replica 1 of 3 is a position"),
            )
            .expect("the digest is lowercase hex"),
        ),
        MAX_KEYRING_LEN,
    );
}

// The ceilings are bounds on the absurd, not on the ordinary: every object this
// build writes is orders of magnitude inside the one for its kind. A ceiling
// that a real object came anywhere near would be a ceiling about to refuse one.
#[test]
fn the_ceilings_admit_the_objects_this_build_writes() {
    for kind in ALL_KINDS {
        let encoded = encode_with(kind);
        let len = encoded.bytes().len() as u64;
        assert!(
            len * 1000 < max_control_object_len(kind),
            "a {kind:?} of {len} bytes is not far enough inside its {} ceiling",
            max_control_object_len(kind),
        );
    }
}

// FM-15, FM-16, FM-17: the three ceilings order the way the payloads do. A
// Keyring grows with the Container count, a record with one batch's Entries, and
// a Snapshot with every Entry the Library holds.
#[test]
fn the_ceilings_order_the_way_the_payloads_grow() {
    const { assert!(MAX_KEYRING_LEN < MAX_JOURNAL_RECORD_LEN) };
    const { assert!(MAX_JOURNAL_RECORD_LEN < MAX_INDEX_SNAPSHOT_LEN) };
    assert_eq!(
        max_control_object_len(ControlObjectKind::ActivationSnapshot),
        max_control_object_len(ControlObjectKind::IndexSnapshot),
        "an activation Snapshot is a Snapshot with two fields more (spec: FM-16)",
    );
}
