//! What the stored Master Key form protects, and what it refuses.
//!
//! Every test here protects at [`argon2_params::CHEAP`] rather than the initial
//! cost: 19 MiB of Argon2id per call would dominate the test run, and what these
//! tests check is the mechanism, not the cost. The initial values have a test of
//! their own next to where they are declared.

use coffret_model::{MasterKey, MasterKeyEpoch};

use super::argon2_params::CHEAP;
use super::{offset, Argon2Params, StoredMasterKey};
use crate::error::Error;

fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x3d; MasterKey::BYTE_LEN])
}

fn epoch(value: u64) -> MasterKeyEpoch {
    MasterKeyEpoch::new(value).expect("the epoch is valid")
}

fn stored() -> StoredMasterKey {
    StoredMasterKey::create_with(CHEAP, b"correct horse", &master_key(), epoch(3))
        .expect("protecting succeeds")
}

// KD-5, KD-7: the stored form encrypts the Master Key and its epoch under the
// Passphrase-derived key, and unlocking with the right Passphrase returns both.
#[test]
fn the_key_and_its_epoch_round_trip() {
    let unlocked = stored()
        .unlock(b"correct horse")
        .expect("the Passphrase is the one that protects it");
    assert_eq!(unlocked.master_key.as_bytes(), master_key().as_bytes());
    assert_eq!(unlocked.epoch, epoch(3));
}

// KD-7: unlocking with another Passphrase fails, and fails as an authentication
// failure rather than by handing back a wrong key.
#[test]
fn another_passphrase_does_not_unlock() {
    assert_eq!(
        stored().unlock(b"correct horst").err(),
        Some(Error::AuthenticationFailed)
    );
}

// KD-5: the salt is per device and drawn fresh, so protecting the same key under
// the same Passphrase twice produces two different stored forms — and both open.
#[test]
fn every_stored_form_draws_its_own_salt() {
    let first = stored();
    let second = stored();
    assert_ne!(first, second);
    assert_ne!(
        &first.as_bytes()[offset::SALT..offset::SALT + StoredMasterKey::SALT_LEN],
        &second.as_bytes()[offset::SALT..offset::SALT + StoredMasterKey::SALT_LEN]
    );
    for form in [first, second] {
        assert!(form.unlock(b"correct horse").is_ok());
    }
}

// KD-5: the Argon2id parameters are recorded in the stored form itself.
#[test]
fn the_parameters_are_recorded_in_the_form() {
    let stored = stored();
    assert_eq!(stored.params(), CHEAP);

    let bytes = stored.as_bytes();
    assert_eq!(&bytes[..5], b"CFMK1");
    assert_eq!(bytes[5], 0x01);
    assert_eq!(bytes[7] as usize, StoredMasterKey::SALT_LEN);
    assert_eq!(&bytes[8..12], &CHEAP.memory_kib().to_be_bytes());
    assert_eq!(&bytes[12..16], &CHEAP.iterations().to_be_bytes());
    assert_eq!(&bytes[16..20], &CHEAP.parallelism().to_be_bytes());
}

// KD-6: the recorded parameters drive the derivation, not this build's current
// policy, so a form written at another cost still unlocks and still reports the
// cost it was written at.
#[test]
fn a_form_written_at_another_cost_still_unlocks() {
    let stronger = Argon2Params::new(16, 3, 1);
    assert_ne!(stronger, Argon2Params::INITIAL);
    assert_ne!(stronger, CHEAP);

    let stored = StoredMasterKey::create_with(stronger, b"pass", &master_key(), epoch(1))
        .expect("protecting succeeds");
    assert_eq!(stored.params(), stronger);

    let unlocked = stored.unlock(b"pass").expect("the Passphrase is right");
    assert_eq!(unlocked.master_key.as_bytes(), master_key().as_bytes());
    assert_eq!(unlocked.epoch, MasterKeyEpoch::FIRST);
}

// KD-7: the recorded parameters are bound in as associated data, so editing them
// fails to unlock instead of quietly deriving under a cheaper cost.
#[test]
fn a_parameter_downgrade_is_detected() {
    let stronger = Argon2Params::new(16, 3, 1);
    let stored = StoredMasterKey::create_with(stronger, b"pass", &master_key(), epoch(1))
        .expect("protecting succeeds");

    // Each edit is to a value Argon2id itself accepts, so what catches it is the
    // authentication and nothing else.
    let downgrades: &[(&str, std::ops::Range<usize>, u32)] = &[
        ("memory", offset::MEMORY_KIB, 8),
        ("iterations", offset::ITERATIONS, 1),
        ("parallelism", offset::PARALLELISM, 2),
    ];
    for (field, range, value) in downgrades {
        let mut bytes = stored.as_bytes().to_vec();
        bytes[range.clone()].copy_from_slice(&value.to_be_bytes());
        let tampered = StoredMasterKey::from_bytes(bytes).expect("the shape is still valid");
        assert_eq!(
            tampered.unlock(b"pass").err(),
            Some(Error::AuthenticationFailed),
            "{field} was not authenticated"
        );
    }
}

// KD-7: the salt and the nonce are authenticated too — the whole plaintext part
// of the form is the associated data.
#[test]
fn editing_the_salt_or_the_nonce_is_detected() {
    let stored = stored();
    let salt_start = offset::SALT;
    let nonce_start = salt_start + StoredMasterKey::SALT_LEN;

    for index in [salt_start, nonce_start] {
        let mut bytes = stored.as_bytes().to_vec();
        bytes[index] ^= 0x01;
        let tampered = StoredMasterKey::from_bytes(bytes).expect("the shape is still valid");
        assert_eq!(
            tampered.unlock(b"correct horse").err(),
            Some(Error::AuthenticationFailed),
            "byte {index} was not authenticated"
        );
    }
}

#[test]
fn editing_the_ciphertext_is_detected() {
    let mut bytes = stored().into_bytes();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let tampered = StoredMasterKey::from_bytes(bytes).expect("the shape is still valid");
    assert_eq!(
        tampered.unlock(b"correct horse").err(),
        Some(Error::AuthenticationFailed)
    );
}

// KD-7: the form is self-contained — the stored bytes are all a reader needs, so
// they survive being written out and read back unchanged.
#[test]
fn the_stored_bytes_are_all_a_reader_needs() {
    let stored = stored();
    let reread = StoredMasterKey::from_bytes(stored.as_bytes().to_vec())
        .expect("the bytes are a valid form");
    assert_eq!(reread, stored);
    assert!(reread.unlock(b"correct horse").is_ok());
}

#[test]
fn bytes_that_are_not_this_form_are_rejected() {
    let mut bytes = stored().into_bytes();
    bytes[..5].copy_from_slice(b"CFRT1");
    assert_eq!(
        StoredMasterKey::from_bytes(bytes),
        Err(Error::UnknownStoredMasterKeyMagic { actual: *b"CFRT1" })
    );
}

#[test]
fn an_unknown_version_is_rejected() {
    let mut bytes = stored().into_bytes();
    bytes[5] = 0x02;
    assert_eq!(
        StoredMasterKey::from_bytes(bytes),
        Err(Error::UnsupportedStoredMasterKeyVersion { actual: 0x02 })
    );
}

#[test]
fn a_truncated_form_is_rejected() {
    let bytes = stored().into_bytes();
    for length in [0, 5, 19, bytes.len() - 1] {
        assert_eq!(
            StoredMasterKey::from_bytes(bytes[..length].to_vec()),
            Err(Error::StoredMasterKeyLengthMismatch),
            "a form of {length} bytes should not parse"
        );
    }
}

#[test]
fn appended_bytes_are_rejected() {
    let mut bytes = stored().into_bytes();
    bytes.push(0);
    assert_eq!(
        StoredMasterKey::from_bytes(bytes),
        Err(Error::StoredMasterKeyLengthMismatch)
    );
}

#[test]
fn a_reserved_byte_that_is_not_zero_is_rejected() {
    let mut bytes = stored().into_bytes();
    bytes[6] = 0x01;
    assert_eq!(
        StoredMasterKey::from_bytes(bytes),
        Err(Error::ReservedNotZero)
    );
}
