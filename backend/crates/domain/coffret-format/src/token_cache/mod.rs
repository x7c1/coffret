//! The form a device's OAuth token cache takes at rest.
//!
//! An adapter that reaches Storage over OAuth keeps a refresh token between
//! runs so that authorizing is a one-time act. That token is a bearer
//! credential for every object this application created in the account: whoever
//! reads it can mint access tokens and fetch every Library's ciphertext kept
//! there without ever touching the device again, and can keep doing so until
//! the grant is revoked. So it is encrypted like everything else coffret
//! writes — under the account's own account-cache key (spec: KD-12, SA-8), a
//! random key a device draws when it first keeps a grant for the account.
//!
//! A Library's previous per-Library cache has this same form, sealed under that
//! Library's `coffret/v1/token-cache` purpose key (KD-4) instead. The form does
//! not say which key sealed it; the two pairs of functions here do, so a caller
//! holding one kind of key cannot open or write the other kind of cache with it.
//! Only the promotion of a previous cache into an account's reads the former.
//!
//! The byte layout is normative in KD-10; this module implements it. The form
//! is self-describing, on the model of the stored Master Key (KD-9), but no
//! Argon2id parameters appear in it: neither key is derived from the
//! Passphrase, so there is nothing to record and nothing to downgrade.
//! Everything before the ciphertext is the associated data, so a file whose
//! header was edited fails to open rather than being read as something it is
//! not.
//!
//! What the plaintext holds is the adapter's business: this module seals opaque
//! bytes and the adapter that owns the cache decides their shape. Like the rest
//! of this crate it does no I/O — bytes in, bytes out — so where a device keeps
//! them, and at what permissions, is a question for the layer that writes them.

use crate::aead::KEY_LEN;
use crate::error::Result;
use crate::nonce;
use crate::purpose::Purpose;
use crate::purpose_key::PurposeKey;

mod decode;
pub use decode::{decode_account_token_cache, decode_token_cache};

mod encode;
pub use encode::{encode_account_token_cache, encode_token_cache};

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

/// The key both halves of this module use, refused unless it was derived for
/// this module's own purpose (spec: KD-4).
fn token_cache_key(key: &PurposeKey) -> Result<&[u8; KEY_LEN]> {
    key.require(Purpose::TokenCache)
}
