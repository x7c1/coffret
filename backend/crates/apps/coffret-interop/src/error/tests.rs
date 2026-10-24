use std::io;

use super::*;

// A cause is the next link of the chain rather than part of this one, so a
// chain printed link by link says each thing once.
#[test]
fn a_cause_is_the_next_link_and_is_said_once() {
    let error = Error::Read {
        path: PathBuf::from("objects/somewhere"),
        cause: io::Error::other("the disk went away"),
    }
    .in_fixture(FixtureKind::Container, "one-file");

    assert_eq!(
        error.chain(),
        vec![
            r#"Container fixture "one-file""#.to_owned(),
            "reading objects/somewhere".to_owned(),
            "the disk went away".to_owned(),
        ],
    );
}

// A failure that says where wraps the failure itself, so a caller can match on
// what is underneath rather than on the words.
#[test]
fn a_failure_in_a_fixture_keeps_the_failure_as_a_value() {
    let error = Error::NoSetDigest.in_fixture(FixtureKind::ControlObject, "keyring-replica");

    let Error::InFixture {
        kind,
        fixture,
        cause,
    } = error
    else {
        panic!("a fixture wraps the failure");
    };
    assert_eq!(kind, FixtureKind::ControlObject);
    assert_eq!(fixture, "keyring-replica");
    assert!(matches!(*cause, Error::NoSetDigest));
}
