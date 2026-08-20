//! What the sealed token cache protects, and what it refuses.

use coffret_model::MasterKey;

use super::{decode_token_cache, encode_token_cache, offset, HEADER_LEN, MAGIC, VERSION};
use crate::aead::TAG_LEN;
use crate::error::Error;
use crate::nonce;

/// A refresh token as Google writes them, so a test that searched for it in the
/// file would find it if anything were written in the clear.
const TOKEN: &[u8] = br#"{"refresh_token":"1//0gSecretRefreshToken"}"#;

fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x3d; MasterKey::BYTE_LEN])
}

fn another_master_key() -> MasterKey {
    MasterKey::from_bytes([0x3e; MasterKey::BYTE_LEN])
}

fn sealed() -> Vec<u8> {
    encode_token_cache(TOKEN, &master_key()).expect("sealing succeeds")
}

// KD-4: the cache is encrypted under the token-cache purpose key, and opens
// again with the Master Key it was sealed under.
#[test]
fn the_cache_round_trips() {
    let opened = decode_token_cache(&sealed(), &master_key()).expect("the Master Key is the one");
    assert_eq!(opened, TOKEN);
}

#[test]
fn an_empty_cache_round_trips() {
    let sealed = encode_token_cache(b"", &master_key()).expect("sealing succeeds");
    assert_eq!(sealed.len(), HEADER_LEN + TAG_LEN);
    assert_eq!(
        decode_token_cache(&sealed, &master_key()).expect("the Master Key is the one"),
        b""
    );
}

// FM-1: what reaches the disk is ciphertext — the credential itself appears
// nowhere in the file.
#[test]
fn the_sealed_form_carries_none_of_the_plaintext() {
    let sealed = sealed();
    assert!(
        !sealed.windows(TOKEN.len()).any(|window| window == TOKEN),
        "the plaintext must not appear in the sealed form"
    );
    assert!(
        !sealed
            .windows(b"1//0gSecretRefreshToken".len())
            .any(|window| window == b"1//0gSecretRefreshToken"),
        "the refresh token must not appear in the sealed form"
    );
}

// FM-1: the nonce is drawn per write, so writing the same cache twice produces
// two different files and both open.
#[test]
fn every_write_draws_its_own_nonce() {
    let first = sealed();
    let second = sealed();
    assert_ne!(
        &first[offset::NONCE..HEADER_LEN],
        &second[offset::NONCE..HEADER_LEN]
    );
    assert_ne!(first, second);
    for bytes in [first, second] {
        assert!(decode_token_cache(&bytes, &master_key()).is_ok());
    }
}

// KD-3: the key is derived from the Master Key, so a cache written on a device
// whose Master Key has been replaced does not open — and says so rather than
// handing back something else.
#[test]
fn another_master_key_does_not_open_the_cache() {
    assert!(matches!(
        decode_token_cache(&sealed(), &another_master_key()),
        Err(Error::AuthenticationFailed)
    ));
}

// FM-1: every part of the file is authenticated — the header ahead of the
// ciphertext as associated data, the rest by the tag.
#[test]
fn editing_any_byte_of_the_message_is_detected() {
    let length = sealed().len();
    let regions = [
        ("the nonce", offset::NONCE),
        ("the ciphertext", HEADER_LEN),
        ("the tag", length - 1),
    ];
    for (region, index) in regions {
        let mut bytes = sealed();
        bytes[index] ^= 0x01;
        assert!(
            matches!(
                decode_token_cache(&bytes, &master_key()),
                Err(Error::AuthenticationFailed)
            ),
            "{region} was not authenticated"
        );
    }
}

// KD-10: the header is checked on its own bytes, before the key is touched, so
// a file that is not this form at all is told apart from one that fails to
// open.
#[test]
fn a_file_that_is_not_this_form_is_rejected_by_its_header() {
    let mut wrong_magic = sealed();
    wrong_magic[0] ^= 0x01;
    assert!(matches!(
        decode_token_cache(&wrong_magic, &master_key()),
        Err(Error::UnknownTokenCacheMagic { .. })
    ));

    let mut wrong_version = sealed();
    wrong_version[offset::VERSION] = VERSION.wrapping_add(1);
    assert!(matches!(
        decode_token_cache(&wrong_version, &master_key()),
        Err(Error::UnsupportedTokenCacheVersion { actual }) if actual == VERSION + 1
    ));

    let mut reserved_set = sealed();
    reserved_set[offset::RESERVED] = 0x01;
    assert!(matches!(
        decode_token_cache(&reserved_set, &master_key()),
        Err(Error::ReservedNotZero)
    ));
}

// A cache written before the file was encrypted at all is JSON, whose first
// bytes are not the magic: it is refused rather than read as tokens.
#[test]
fn a_plaintext_cache_is_refused() {
    assert!(matches!(
        decode_token_cache(TOKEN, &master_key()),
        Err(Error::UnknownTokenCacheMagic { .. })
    ));
}

#[test]
fn a_truncated_file_is_rejected() {
    let sealed = sealed();
    for length in [0, MAGIC.len(), HEADER_LEN, HEADER_LEN + TAG_LEN - 1] {
        assert!(
            matches!(
                decode_token_cache(&sealed[..length], &master_key()),
                Err(Error::TokenCacheTooShort { .. })
            ),
            "a file of {length} bytes should not parse"
        );
    }
}

// KD-10: the layout the register lays down, checked on the bytes themselves so
// that a change to it is a change to this test.
#[test]
fn the_layout_is_the_one_written_down() {
    let sealed = sealed();
    assert_eq!(&sealed[..MAGIC.len()], b"CFTC1");
    assert_eq!(sealed[offset::VERSION], 0x01);
    assert_eq!(sealed[offset::RESERVED], 0x00);
    assert_eq!(offset::NONCE + nonce::LEN, HEADER_LEN);
    assert_eq!(sealed.len(), HEADER_LEN + TOKEN.len() + TAG_LEN);
}
