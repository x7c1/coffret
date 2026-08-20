//! The form a device's OAuth token cache takes at rest, under the Master Key.
//!
//! An adapter that reaches Storage over OAuth keeps a refresh token between
//! runs so that authorizing is a one-time act. That token is a bearer
//! credential for every object coffret put in the Library: whoever reads it can
//! mint access tokens and fetch the whole Library's ciphertext without ever
//! touching the device again, and can keep doing so until the grant is revoked.
//! So it is encrypted like everything else coffret writes — under the
//! `coffret/v1/token-cache` purpose key (KD-4), the first key here to protect
//! device-local state rather than a Storage Object.
//!
//! The byte layout is normative in KD-10; this module implements it. The form
//! is self-describing, on the model of the stored Master Key (KD-9), but no
//! Argon2id parameters appear in it: this key comes from the Master Key rather
//! than from the Passphrase, so there is nothing to record and nothing to
//! downgrade. Everything before the ciphertext is the associated data, so a
//! file whose header was edited fails to open rather than being read as
//! something it is not.
//!
//! What the plaintext holds is the adapter's business: this module seals opaque
//! bytes and the adapter that owns the cache decides their shape. Like the rest
//! of this crate it does no I/O — bytes in, bytes out — so where a device keeps
//! them, and at what permissions, is a question for the layer that writes them.

use coffret_model::MasterKey;

use crate::aead::KEY_LEN;
use crate::nonce;
use crate::purpose::Purpose;
use crate::purpose_key::PurposeKey;

mod decode;
pub use decode::decode_token_cache;

mod encode;
pub use encode::encode_token_cache;

mod layout;
use layout::Layout;

#[cfg(test)]
mod tests;

/// Length of the magic in bytes.
pub(crate) const MAGIC_LEN: usize = 5;

/// The bytes a sealed token cache starts with.
const MAGIC: [u8; MAGIC_LEN] = *b"CFTC1";

/// The version this crate writes and reads.
const VERSION: u8 = 0x01;

/// Offsets of the plaintext part of the form.
mod offset {
    pub(super) const VERSION: usize = 5;
    pub(super) const RESERVED: usize = 6;
    /// Where the nonce starts, and therefore how long the fixed part is.
    pub(super) const NONCE: usize = 7;
}

/// Length of everything before the ciphertext, which is also the associated
/// data of the encryption.
const HEADER_LEN: usize = offset::NONCE + nonce::LEN;

/// The key both halves of this module use, derived where the Master Key is.
///
/// Callers hand over the Master Key rather than a derived key: the derivation
/// is the one in [`PurposeKey`], and a token cache is the only thing this key
/// ever opens, so there is no reason for raw derived bytes to travel.
fn token_cache_key(master_key: &MasterKey) -> [u8; KEY_LEN] {
    *PurposeKey::derive(master_key, Purpose::TokenCache)
        .require(Purpose::TokenCache)
        .expect("the key was derived for this very purpose")
}
