//! What every name form spells, admits, and refuses.

use super::*;

/// Every control-object kind, so a pairing the admission table leaves out
/// is a pairing a test still visits.
const ALL_KINDS: [ControlObjectKind; 4] = ControlObjectKind::ALL;

fn keyring(index: u16, count: u16) -> ControlObjectName {
    ControlObjectName::keyring_replica(
        Generation::new(12),
        "a1b2",
        ReplicaPosition::new(index, count).expect("the position is valid"),
    )
    .expect("a lowercase hex digest is a valid one")
}

// FM-12: control objects are named `head-<generation>.cfrt`,
// `idx-<generation>.cfrt`, and
// `key-<generation>-<set_digest>-r<index>-of-<count>.cfrt`.
#[test]
fn names_match_the_forms_the_rule_defines() {
    assert_eq!(
        ControlObjectName::head(Generation::new(4)).to_string(),
        "head-4.cfrt"
    );
    assert_eq!(
        ControlObjectName::index_snapshot(Generation::new(4)).to_string(),
        "idx-4.cfrt"
    );
    assert_eq!(keyring(1, 3).to_string(), "key-12-a1b2-r1-of-3.cfrt");
}

// FM-12, FM-13: the successor of a head takes the head's generation plus 1,
// whichever kind ends up winning the slot.
#[test]
fn the_successor_of_a_head_is_the_next_generation() {
    assert_eq!(
        ControlObjectName::successor_of(Generation::new(4))
            .expect("generation 4 has a successor")
            .to_string(),
        "head-5.cfrt"
    );
    let result = ControlObjectName::successor_of(Generation::new(u64::MAX));
    assert!(
        matches!(result, Err(Error::GenerationOutOfRange)),
        "expected the last generation to name no successor, got {result:?}"
    );
}

// FM-12: every form round-trips, so a name written by one device is read
// back to the same values by another.
#[test]
fn every_form_round_trips() {
    let names = [
        ControlObjectName::head(Generation::FIRST),
        ControlObjectName::head(Generation::new(u64::MAX)),
        ControlObjectName::index_snapshot(Generation::new(9)),
        keyring(0, 1),
        keyring(2, 3),
    ];
    for name in names {
        assert_eq!(
            ControlObjectName::parse(&name.to_string()).expect("a name's own text parses back"),
            name
        );
    }
}

// FM-12: heads and Index Snapshots use replica index 0, count 1.
#[test]
fn single_written_forms_report_replica_zero_of_one() {
    for name in [
        ControlObjectName::head(Generation::new(1)),
        ControlObjectName::index_snapshot(Generation::new(1)),
    ] {
        assert_eq!(name.replica(), ReplicaPosition::SINGLE);
        assert_eq!(name.replica().index(), 0);
        assert_eq!(name.replica().count(), 1);
        assert_eq!(name.set_digest(), None);
    }
}

// FM-12: the admission table — `head-` admits a Journal record or an
// activation Index Snapshot, `idx-` only an ordinary Index Snapshot, `key-`
// only a Keyring replica.
#[test]
fn each_name_form_admits_exactly_the_kinds_the_table_lists() {
    let admitted: [(ControlObjectName, &[ControlObjectKind]); 3] = [
        (
            ControlObjectName::head(Generation::FIRST),
            &[
                ControlObjectKind::Journal,
                ControlObjectKind::ActivationSnapshot,
            ],
        ),
        (
            ControlObjectName::index_snapshot(Generation::FIRST),
            &[ControlObjectKind::IndexSnapshot],
        ),
        (keyring(0, 1), &[ControlObjectKind::Keyring]),
    ];
    for (name, kinds) in admitted {
        for kind in ALL_KINDS {
            assert_eq!(
                name.admits(kind),
                kinds.contains(&kind),
                "{name} and {kind:?}"
            );
        }
    }
}

#[test]
fn a_keyring_replica_name_carries_its_digest() {
    assert_eq!(keyring(0, 1).set_digest(), Some("a1b2"));
}

#[test]
fn names_outside_the_forms_are_rejected() {
    let names = [
        "head-4",                   // no extension
        "head-.cfrt",               // no generation
        "head-04.cfrt",             // a second spelling of generation 4
        "head-4x.cfrt",             // not a number
        "head--4.cfrt",             // signed, in effect
        "log-4.cfrt",               // not a role coffret writes
        "key-12-a1b2-r1-of.cfrt",   // no replica count
        "key-12-a1b2-r1-to-3.cfrt", // not the `of` separator
        "key-12-a1b2-1-of-3.cfrt",  // no `r` on the index
        "0011.cfrt",                // a Container, not a control object
    ];
    for name in names {
        match ControlObjectName::parse(name) {
            // The error quotes the name as it was presented, so a log says
            // which of the candidates was rejected.
            Err(Error::MalformedObjectName { name: reported }) => {
                assert_eq!(reported, name, "the error should quote {name}");
            }
            other => panic!("{name} should not parse, got {other:?}"),
        }
    }
}

// FM-12: a name whose shape is a replica's but whose digest field is not the
// lowercase hex token is a Keyring replica with a corrupt field, not an object
// of some other form. A reader scanning Storage acts differently on the two,
// so the two refusals stay apart.
#[test]
fn a_replica_name_with_a_bad_digest_is_refused_for_the_digest() {
    let names = [
        ("key-12-zz-r1-of-3.cfrt", "zz"),     // not hex
        ("key-12-A1B2-r1-of-3.cfrt", "A1B2"), // not lowercase
        ("key-12--r1-of-3.cfrt", ""),         // no digest at all
    ];
    for (name, digest) in names {
        match ControlObjectName::parse(name) {
            Err(Error::InvalidSetDigest { digest: reported }) => {
                assert_eq!(reported, digest, "the error should quote {name}'s digest");
            }
            other => panic!("{name} should be refused for its digest, got {other:?}"),
        }
    }
}

// FM-12: a replica index outside its count names no replica, whatever the
// rest of the name says.
#[test]
fn a_name_with_an_inconsistent_replica_position_is_rejected() {
    let result = ControlObjectName::parse("key-12-a1b2-r3-of-3.cfrt");
    assert!(
        matches!(
            result,
            Err(Error::InvalidReplicaPosition { index: 3, count: 3 })
        ),
        "expected replica 3 of 3 to be rejected, got {result:?}"
    );
}
