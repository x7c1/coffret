//! What the stored Master Key form protects, and what it refuses.
//!
//! Every test here protects at [`argon2_params::CHEAP`] rather than the initial
//! cost: 19 MiB of Argon2id per call would dominate the test run, and what these
//! tests check is the mechanism, not the cost. The initial values have a test of
//! their own next to where they are declared.

use coffret_model::{MasterKey, MasterKeyEpoch, Passphrase, MAX_FORMAT_INTEGER};

use super::argon2_params::CHEAP;
use super::{offset, Argon2Params, StoredMasterKey, PLAINTEXT_LEN};
use crate::aead::Cipher;
use crate::error::Error;
use crate::nonce;

fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x3d; MasterKey::BYTE_LEN])
}

/// The Passphrase these cases protect and open a stored form under.
fn passphrase(bytes: &[u8]) -> Passphrase {
    Passphrase::from_bytes(bytes.to_vec())
}

fn epoch(value: u64) -> MasterKeyEpoch {
    MasterKeyEpoch::new(value).expect("the epoch is valid")
}

fn stored() -> StoredMasterKey {
    StoredMasterKey::create_with(
        CHEAP,
        &passphrase(b"correct horse"),
        &master_key(),
        epoch(3),
    )
    .expect("protecting succeeds")
}

/// A form whose 8 epoch bytes spell `value`, sealed the way a writer would.
///
/// [`StoredMasterKey::create_with`] takes a [`MasterKeyEpoch`], so a number that
/// numbers no epoch cannot reach the plaintext through it. Resealing here puts
/// one there under the same associated data, which leaves the form authentic —
/// so what the epoch cases below observe is the epoch's own refusal rather than
/// a tag that failed to verify.
fn stored_with_epoch_bytes(value: u64) -> StoredMasterKey {
    let form = stored();
    let bytes = form.as_bytes();
    let salt_end = offset::SALT + StoredMasterKey::SALT_LEN;
    let nonce_end = salt_end + nonce::LEN;
    let nonce: [u8; nonce::LEN] = bytes[salt_end..nonce_end]
        .try_into()
        .expect("the slice is nonce::LEN long");
    let protection_key = CHEAP
        .derive(
            &passphrase(b"correct horse"),
            &bytes[offset::SALT..salt_end],
        )
        .expect("the recorded parameters are valid");

    let mut plaintext = Vec::with_capacity(PLAINTEXT_LEN);
    plaintext.extend_from_slice(master_key().as_bytes());
    plaintext.extend_from_slice(&value.to_be_bytes());

    let mut resealed = bytes[..nonce_end].to_vec();
    Cipher::new(&protection_key)
        .seal(&nonce, &bytes[..nonce_end], &mut plaintext, &mut resealed)
        .expect("sealing succeeds");
    StoredMasterKey::from_bytes(resealed).expect("the form's shape is unchanged")
}

// KD-9, FM-13: epochs are numbered from 1, so a form whose epoch bytes spell 0
// carries no pair a Library could have written — and it says so as a stored
// Master Key rather than as a domain value somebody built wrong.
#[test]
fn a_stored_epoch_below_one_is_refused() {
    let result = stored_with_epoch_bytes(0).unlock(&passphrase(b"correct horse"));
    assert!(
        matches!(
            result,
            Err(Error::StoredMasterKeyEpochOutOfRange { epoch: 0 })
        ),
        "expected epoch 0 to be refused, got {result:?}"
    );
}

// KD-9, FM-19: the epoch bytes spell any `u64`, and the format admits only the
// numbers below 2^63, so a form carrying a larger one names no epoch either —
// the same refusal epoch 0 gets, in this layer's own vocabulary rather than the
// model's.
#[test]
fn a_stored_master_key_epoch_past_the_formats_integer_range_is_refused() {
    let past_the_bound = MAX_FORMAT_INTEGER + 1;
    let result = stored_with_epoch_bytes(past_the_bound).unlock(&passphrase(b"correct horse"));
    assert!(
        matches!(
            result,
            Err(Error::StoredMasterKeyEpochOutOfRange { epoch }) if epoch == past_the_bound
        ),
        "expected an epoch of 2^63 to be refused, got {result:?}"
    );

    // The bound itself is an epoch a Library can reach, so it still unlocks.
    let unlocked = stored_with_epoch_bytes(MAX_FORMAT_INTEGER)
        .unlock(&passphrase(b"correct horse"))
        .expect("the bound numbers an epoch");
    assert_eq!(unlocked.epoch, epoch(MAX_FORMAT_INTEGER));
}

// KD-5, KD-7: the stored form encrypts the Master Key and its epoch under the
// Passphrase-derived key, and unlocking with the right Passphrase returns both.
#[test]
fn the_key_and_its_epoch_round_trip() {
    let unlocked = stored()
        .unlock(&passphrase(b"correct horse"))
        .expect("the Passphrase is the one that protects it");
    assert_eq!(unlocked.master_key.as_bytes(), master_key().as_bytes());
    assert_eq!(unlocked.epoch, epoch(3));
}

// KD-7: unlocking with another Passphrase fails, and fails as an authentication
// failure rather than by handing back a wrong key.
#[test]
fn another_passphrase_does_not_unlock() {
    let result = stored().unlock(&passphrase(b"correct horst"));
    assert!(
        matches!(result, Err(Error::AuthenticationFailed)),
        "expected another Passphrase to fail authentication, got {result:?}"
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
        assert!(form.unlock(&passphrase(b"correct horse")).is_ok());
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

    let stored =
        StoredMasterKey::create_with(stronger, &passphrase(b"pass"), &master_key(), epoch(1))
            .expect("protecting succeeds");
    assert_eq!(stored.params(), stronger);

    let unlocked = stored
        .unlock(&passphrase(b"pass"))
        .expect("the Passphrase is right");
    assert_eq!(unlocked.master_key.as_bytes(), master_key().as_bytes());
    assert_eq!(unlocked.epoch, MasterKeyEpoch::FIRST);
}

// KD-7: the recorded parameters are bound in as associated data, so editing them
// fails to unlock instead of quietly deriving under a cheaper cost.
#[test]
fn a_parameter_downgrade_is_detected() {
    let stronger = Argon2Params::new(16, 3, 1);
    let stored =
        StoredMasterKey::create_with(stronger, &passphrase(b"pass"), &master_key(), epoch(1))
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
        let result = tampered.unlock(&passphrase(b"pass"));
        assert!(
            matches!(result, Err(Error::AuthenticationFailed)),
            "{field} was not authenticated, got {result:?}"
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
        let result = tampered.unlock(&passphrase(b"correct horse"));
        assert!(
            matches!(result, Err(Error::AuthenticationFailed)),
            "byte {index} was not authenticated, got {result:?}"
        );
    }
}

#[test]
fn editing_the_ciphertext_is_detected() {
    let mut bytes = stored().into_bytes();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let tampered = StoredMasterKey::from_bytes(bytes).expect("the shape is still valid");
    let result = tampered.unlock(&passphrase(b"correct horse"));
    assert!(
        matches!(result, Err(Error::AuthenticationFailed)),
        "expected an edited ciphertext to fail authentication, got {result:?}"
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
    assert!(reread.unlock(&passphrase(b"correct horse")).is_ok());
}

#[test]
fn bytes_that_are_not_this_form_are_rejected() {
    let mut bytes = stored().into_bytes();
    bytes[..5].copy_from_slice(b"CFRT1");
    let result = StoredMasterKey::from_bytes(bytes);
    assert!(
        matches!(
            result,
            Err(Error::UnknownStoredMasterKeyMagic { actual }) if actual == *b"CFRT1"
        ),
        "expected the Container magic to name no stored Master Key, got {result:?}"
    );
}

#[test]
fn an_unknown_version_is_rejected() {
    let mut bytes = stored().into_bytes();
    bytes[5] = 0x02;
    let result = StoredMasterKey::from_bytes(bytes);
    assert!(
        matches!(
            result,
            Err(Error::UnsupportedStoredMasterKeyVersion { actual: 0x02 })
        ),
        "expected version 0x02 to be unreadable, got {result:?}"
    );
}

#[test]
fn a_truncated_form_is_rejected() {
    let bytes = stored().into_bytes();
    for length in [0, 5, 19, bytes.len() - 1] {
        let result = StoredMasterKey::from_bytes(bytes[..length].to_vec());
        assert!(
            matches!(result, Err(Error::StoredMasterKeyLengthMismatch)),
            "a form of {length} bytes should not parse, got {result:?}"
        );
    }
}

#[test]
fn appended_bytes_are_rejected() {
    let mut bytes = stored().into_bytes();
    bytes.push(0);
    let result = StoredMasterKey::from_bytes(bytes);
    assert!(
        matches!(result, Err(Error::StoredMasterKeyLengthMismatch)),
        "expected a trailing byte to be refused, got {result:?}"
    );
}

#[test]
fn a_reserved_byte_that_is_not_zero_is_rejected() {
    let mut bytes = stored().into_bytes();
    bytes[6] = 0x01;
    let result = StoredMasterKey::from_bytes(bytes);
    assert!(
        matches!(result, Err(Error::ReservedNotZero)),
        "expected a non-zero reserved byte to be refused, got {result:?}"
    );
}
