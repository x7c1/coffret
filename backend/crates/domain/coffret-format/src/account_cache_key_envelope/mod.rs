//! The account-cache key envelope: an account's account-cache key, wrapped
//! under one Library's `coffret/v1/account-cache-wrap` purpose key and bound to
//! the device-local account name.
//!
//! The byte layout is normative in KD-12, whose statement lives in this
//! module's tests; this module implements it beside the token cache it opens
//! (spec: KD-10). It is a sibling of that form and follows the same reading
//! discipline — the length, the magic, the version and the reserved byte are
//! checked on the bytes themselves before the key is touched — with one thing
//! the cache does not have: the account name is part of the associated data
//! without being written into the envelope, so an envelope opened for another
//! account's name fails to authenticate rather than yielding a key.
//!
//! It is not a Key Envelope (FM-14), which wraps a Container Key and lives in
//! the Keyring on Storage: this one is device-local and never uploaded. The
//! account-cache key is in the clear on both sides of this module, and both
//! times it is in a buffer this module owns and wipes before it returns
//! (spec: DK-7).

use coffret_model::AccountCacheKey;
use zeroize::Zeroizing;

use crate::aead::{Cipher, KEY_LEN, TAG_LEN};
use crate::error::{Error, Result};
use crate::nonce;
use crate::purpose::Purpose;
use crate::purpose_key::PurposeKey;

#[cfg(test)]
mod tests;

/// Length of the magic in bytes.
pub(crate) const MAGIC_LEN: usize = 5;

/// The bytes an account-cache key envelope starts with.
const MAGIC: [u8; MAGIC_LEN] = *b"CFAK1";

/// The version this crate writes and reads.
const VERSION: u8 = 0x01;

/// Offsets of the fixed part of the form.
mod offset {
    pub(super) const VERSION: usize = 5;
    pub(super) const RESERVED: usize = 6;
    /// Where the nonce starts.
    pub(super) const NONCE: usize = 7;
}

/// Length of everything before the ciphertext, which is where the associated
/// data starts before the account name is put after it.
const HEADER_LEN: usize = offset::NONCE + nonce::LEN;

/// Length of every envelope: the fixed part, the wrapped key, and its tag.
pub const ACCOUNT_CACHE_KEY_ENVELOPE_LEN: usize = HEADER_LEN + AccountCacheKey::BYTE_LEN + TAG_LEN;

/// Wraps an account's account-cache key into the envelope one Library holds
/// for it, bound to the account name the Library references it by.
///
/// A key derived for any other purpose is refused rather than used (KD-4). The
/// nonce is drawn fresh on every call, since one Library's purpose key covers
/// every envelope it ever writes.
pub fn encode_account_cache_key_envelope(
    account_cache_key: &AccountCacheKey,
    key: &PurposeKey,
    account_name: &str,
) -> Result<Vec<u8>> {
    let cipher = Cipher::new(wrap_key(key)?);
    let nonce = nonce::random()?;

    let mut envelope = Vec::with_capacity(ACCOUNT_CACHE_KEY_ENVELOPE_LEN);
    envelope.extend_from_slice(&MAGIC);
    envelope.push(VERSION);
    envelope.push(0); // reserved
    envelope.extend_from_slice(&nonce);

    let associated_data = associated_data(&envelope, account_name);
    // `seal` encrypts in place, so this copy of the key is ciphertext by the
    // time the call returns — but it is named and wiped rather than left as a
    // temporary, so a failure part-way through leaves nothing readable.
    let mut plaintext = Zeroizing::new(account_cache_key.as_bytes().to_vec());
    cipher.seal(&nonce, &associated_data, &mut plaintext, &mut envelope)?;
    Ok(envelope)
}

/// Opens the envelope one Library holds for an account, for the account name
/// the Library references it by.
///
/// Every refusal yields no key material at all: a length, a magic, a version or
/// a reserved byte that is not this form's is refused by its bytes, and an
/// envelope sealed under another Library's purpose key or for another account
/// name fails to authenticate.
pub fn decode_account_cache_key_envelope(
    bytes: &[u8],
    key: &PurposeKey,
    account_name: &str,
) -> Result<AccountCacheKey> {
    let cipher = Cipher::new(wrap_key(key)?);

    // Shape first, key afterwards: bytes that are not this form at all are told
    // apart from bytes that are and fail to open.
    if bytes.len() != ACCOUNT_CACHE_KEY_ENVELOPE_LEN {
        return Err(Error::AccountCacheKeyEnvelopeLength {
            actual: bytes.len(),
        });
    }
    let magic: [u8; MAGIC_LEN] = bytes[..MAGIC_LEN]
        .try_into()
        .expect("the slice is MAGIC_LEN long");
    if magic != MAGIC {
        return Err(Error::UnknownAccountCacheKeyEnvelopeMagic { actual: magic });
    }
    let version = bytes[offset::VERSION];
    if version != VERSION {
        return Err(Error::UnsupportedAccountCacheKeyEnvelopeVersion { actual: version });
    }
    if bytes[offset::RESERVED] != 0 {
        return Err(Error::ReservedNotZero);
    }
    let nonce: [u8; nonce::LEN] = bytes[offset::NONCE..HEADER_LEN]
        .try_into()
        .expect("the slice is nonce::LEN long");

    let associated_data = associated_data(&bytes[..HEADER_LEN], account_name);
    // The key in the clear, until it is read into the type that guards it. The
    // buffer the cipher allocated is taken over rather than copied out of, so
    // there is one plaintext copy and it is wiped when this call ends.
    let plaintext = Zeroizing::new(cipher.open(&nonce, &associated_data, &bytes[HEADER_LEN..])?);
    let key: [u8; AccountCacheKey::BYTE_LEN] = plaintext[..]
        .try_into()
        .expect("the envelope's fixed length leaves exactly an account-cache key");
    Ok(AccountCacheKey::from_bytes(key))
}

/// The fixed part of the envelope followed by the account name's UTF-8 bytes.
///
/// The fixed part has one length, so the concatenation has one reading, and the
/// name is bound without being written into the envelope.
fn associated_data(header: &[u8], account_name: &str) -> Vec<u8> {
    let mut associated_data = Vec::with_capacity(header.len() + account_name.len());
    associated_data.extend_from_slice(header);
    associated_data.extend_from_slice(account_name.as_bytes());
    associated_data
}

/// The key both halves of this module use, refused unless it was derived for
/// this module's own purpose (spec: KD-4).
fn wrap_key(key: &PurposeKey) -> Result<&[u8; KEY_LEN]> {
    key.require(Purpose::AccountCacheWrap)
}
