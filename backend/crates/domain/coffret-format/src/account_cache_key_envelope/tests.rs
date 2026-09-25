//! What the account-cache key envelope protects, and what it refuses.
//!
//! KD-12: An account-cache key is 256 bits drawn from the operating system's
//! CSPRNG when a device first keeps a grant for the account. It is derived from
//! no Master Key, which is what lets Libraries holding different Master Keys
//! open one account's cache, each through its own envelope (SA-9). An
//! account-cache key envelope is one self-describing byte string:
//!
//! ```text
//! offset  size  field
//! ------  ----  -----
//! 0       5     magic = "CFAK1"
//! 5       1     format version = 0x01
//! 6       1     reserved = 0x00
//! 7       24    nonce (random, drawn per write)
//! 31      32    ciphertext of the account-cache key
//! 63      16    tag
//! ```
//!
//! The encryption is XChaCha20-Poly1305 under the Library's
//! `coffret/v1/account-cache-wrap` purpose key (KD-3, KD-4). The associated
//! data is everything before the ciphertext followed by the device-local
//! account name's UTF-8 bytes; the part before the name has a fixed length, so
//! the concatenation has one reading, and the name is bound without being
//! written into the envelope. A reader rejects an unknown magic, an unknown
//! version, a non-zero reserved byte, or a total length other than 79; a file
//! that fails any of these checks, or fails to authenticate under the name it
//! is opened for, is reported as an unreadable envelope, and yields no key
//! material at all.
//!
//! - The envelope is device-local and never uploaded — it is not a Storage
//!   Object and not a Key Envelope, which wraps a Container Key (FM-14) — so
//!   KD-8 is untouched by it: nothing here is Passphrase-derived and nothing
//!   here reaches Storage.
//!
//! The cases below sample it: the drawing in `generate_account_cache_key`'s own
//! case, and the envelope's form, its binding, and its refusals here. The
//! device layer's cases sample the rest — an envelope that does not open is
//! reported as an unreadable one there, never as a Library with no account.

use coffret_model::{AccountCacheKey, MasterKey};

use super::{
    decode_account_cache_key_envelope, encode_account_cache_key_envelope, offset,
    ACCOUNT_CACHE_KEY_ENVELOPE_LEN, HEADER_LEN, MAGIC, VERSION,
};
use crate::aead::TAG_LEN;
use crate::error::Error;
use crate::nonce;
use crate::purpose::{Purpose, ALL};
use crate::purpose_key::PurposeKey;

/// The account name every case binds its envelope to.
const ACCOUNT: &str = "work";

/// The key a Library wraps its account's key under, derived as a device
/// derives it: from that Library's Master Key, for this purpose and no other.
fn wrap_key(master_key_byte: u8) -> PurposeKey {
    PurposeKey::derive(
        &MasterKey::from_bytes([master_key_byte; MasterKey::BYTE_LEN]),
        Purpose::AccountCacheWrap,
    )
}

/// An account-cache key whose bytes a case can recognise.
fn account_key() -> AccountCacheKey {
    AccountCacheKey::from_bytes([0x5a; AccountCacheKey::BYTE_LEN])
}

fn sealed() -> Vec<u8> {
    encode_account_cache_key_envelope(&account_key(), &wrap_key(0x11), ACCOUNT)
        .expect("sealing succeeds")
}

// KD-12: the layout the register lays down, checked on the bytes themselves so
// that a change to it is a change to this test.
#[test]
fn the_layout_is_the_one_written_down() {
    let sealed = sealed();
    assert_eq!(sealed.len(), 79);
    assert_eq!(ACCOUNT_CACHE_KEY_ENVELOPE_LEN, 79);
    assert_eq!(&sealed[..MAGIC.len()], b"CFAK1");
    assert_eq!(sealed[offset::VERSION], 0x01);
    assert_eq!(sealed[offset::RESERVED], 0x00);
    assert_eq!(offset::NONCE + nonce::LEN, HEADER_LEN);
    assert_eq!(HEADER_LEN, 31);
    assert_eq!(
        sealed.len(),
        HEADER_LEN + AccountCacheKey::BYTE_LEN + TAG_LEN
    );
}

// KD-12: the envelope carries the account-cache key, and opening it under the
// same purpose key and account name returns exactly that key — and the key
// appears nowhere in the envelope in the clear.
#[test]
fn the_envelope_round_trips_and_carries_none_of_the_key() {
    let sealed = sealed();
    let opened = decode_account_cache_key_envelope(&sealed, &wrap_key(0x11), ACCOUNT)
        .expect("the key and the name are the ones it was sealed with");
    assert_eq!(opened.as_bytes(), account_key().as_bytes());
    assert!(!sealed
        .windows(AccountCacheKey::BYTE_LEN)
        .any(|window| window == account_key().as_bytes()));
}

// KD-12: the name is bound without being written into the envelope.
#[test]
fn the_account_name_is_bound_and_not_written() {
    let name = "a-name-nobody-else-uses";
    let sealed = encode_account_cache_key_envelope(&account_key(), &wrap_key(0x11), name)
        .expect("sealing succeeds");
    assert_eq!(sealed.len(), 79, "the name does not change the length");
    assert!(!sealed
        .windows(name.len())
        .any(|window| window == name.as_bytes()));
}

// KD-12, SA-9: an envelope opened under a name other than the one it was sealed
// with fails to authenticate, so it cannot be carried over to stand for another
// account — a prefix or an extension of the name included.
#[test]
fn an_envelope_does_not_open_for_another_account_name() {
    for other in ["home", "wor", "work2", "Work", ""] {
        assert!(
            matches!(
                decode_account_cache_key_envelope(&sealed(), &wrap_key(0x11), other),
                Err(Error::AuthenticationFailed)
            ),
            "an envelope for {ACCOUNT:?} must not open for {other:?}"
        );
    }
}

// KD-12, SA-9: an envelope is one Library's, under that Library's own purpose
// key — another Library's key does not open it.
#[test]
fn an_envelope_does_not_open_under_another_library_s_key() {
    assert!(matches!(
        decode_account_cache_key_envelope(&sealed(), &wrap_key(0x22), ACCOUNT),
        Err(Error::AuthenticationFailed)
    ));
}

// KD-12: the nonce is drawn per write, so two envelopes of one key differ and
// both open.
#[test]
fn every_write_draws_its_own_nonce() {
    let first = sealed();
    let second = sealed();
    assert_ne!(
        &first[offset::NONCE..HEADER_LEN],
        &second[offset::NONCE..HEADER_LEN]
    );
    for envelope in [first, second] {
        assert!(decode_account_cache_key_envelope(&envelope, &wrap_key(0x11), ACCOUNT).is_ok());
    }
}

// KD-12: every byte is authenticated — the fixed part as associated data, the
// rest by the tag.
#[test]
fn editing_any_byte_is_detected() {
    let regions = [
        ("the nonce", offset::NONCE),
        ("the ciphertext", HEADER_LEN),
        ("the tag", ACCOUNT_CACHE_KEY_ENVELOPE_LEN - 1),
    ];
    for (region, index) in regions {
        let mut bytes = sealed();
        bytes[index] ^= 0x01;
        assert!(
            matches!(
                decode_account_cache_key_envelope(&bytes, &wrap_key(0x11), ACCOUNT),
                Err(Error::AuthenticationFailed)
            ),
            "{region} was not authenticated"
        );
    }
}

// KD-12: a reader rejects an unknown magic, an unknown version, a non-zero
// reserved byte, or a total length other than 79, by the bytes and before the
// key is touched.
#[test]
fn a_file_that_is_not_this_form_is_rejected_by_its_bytes() {
    let mut wrong_magic = sealed();
    wrong_magic[0] ^= 0x01;
    assert!(matches!(
        decode_account_cache_key_envelope(&wrong_magic, &wrap_key(0x11), ACCOUNT),
        Err(Error::UnknownAccountCacheKeyEnvelopeMagic { .. })
    ));

    let mut wrong_version = sealed();
    wrong_version[offset::VERSION] = VERSION.wrapping_add(1);
    assert!(matches!(
        decode_account_cache_key_envelope(&wrong_version, &wrap_key(0x11), ACCOUNT),
        Err(Error::UnsupportedAccountCacheKeyEnvelopeVersion { actual }) if actual == VERSION + 1
    ));

    let mut reserved_set = sealed();
    reserved_set[offset::RESERVED] = 0x01;
    assert!(matches!(
        decode_account_cache_key_envelope(&reserved_set, &wrap_key(0x11), ACCOUNT),
        Err(Error::ReservedNotZero)
    ));

    let whole = sealed();
    let mut longer = whole.clone();
    longer.push(0);
    for (what, bytes) in [
        ("empty", &[][..]),
        ("one short", &whole[..whole.len() - 1]),
        ("one long", &longer[..]),
    ] {
        assert!(
            matches!(
                decode_account_cache_key_envelope(bytes, &wrap_key(0x11), ACCOUNT),
                Err(Error::AccountCacheKeyEnvelopeLength { actual }) if actual == bytes.len()
            ),
            "an envelope {what} must be refused by its length"
        );
    }
}

// KD-4: a key derived for another purpose wraps nothing here and opens nothing
// here, whatever its bytes would do to the ciphertext.
#[test]
fn a_key_derived_for_another_purpose_is_refused() {
    let master_key = MasterKey::from_bytes([0x11; MasterKey::BYTE_LEN]);
    for purpose in ALL {
        if purpose == Purpose::AccountCacheWrap {
            continue;
        }
        let key = PurposeKey::derive(&master_key, purpose);
        assert!(
            matches!(
                encode_account_cache_key_envelope(&account_key(), &key, ACCOUNT),
                Err(Error::WrongPurposeKey {
                    expected: Purpose::AccountCacheWrap,
                    actual,
                }) if actual == purpose
            ),
            "{purpose} should not wrap an account-cache key"
        );
        assert!(
            matches!(
                decode_account_cache_key_envelope(&sealed(), &key, ACCOUNT),
                Err(Error::WrongPurposeKey {
                    expected: Purpose::AccountCacheWrap,
                    actual,
                }) if actual == purpose
            ),
            "{purpose} should not open an account-cache key envelope"
        );
    }
}
