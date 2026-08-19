//! The form a device's Master Key takes at rest, under its Passphrase.
//!
//! ```text
//! offset  size  field
//! ------  ----  -----
//! 0       5     magic = "CFMK1"
//! 5       1     format version = 0x01
//! 6       1     reserved = 0x00
//! 7       1     salt length S
//! 8       4     Argon2id memory cost in KiB
//! 12      4     Argon2id iterations
//! 16      4     Argon2id parallelism
//! 20      S     Argon2id salt (per device, random)
//! 20+S    24    nonce (random)
//! 44+S    40    ciphertext of Master Key(32) ‖ epoch(8)
//! 84+S    16    tag
//! ```
//!
//! Everything before the ciphertext is the associated data, so the recorded
//! Argon2id parameters and the salt are authenticated: unlocking detects both
//! tampering and an attempt to talk the reader into a cheaper derivation than the
//! writer used.
//!
//! The form is self-contained and portable — unlocking needs only these bytes and
//! the Passphrase — and it never reaches Storage: nothing Passphrase-derived
//! does. This module deals in bytes only; where a device keeps them is a question
//! for the layer that does I/O.

use coffret_model::MasterKey;

use crate::error::Result;

mod argon2_params;
pub use argon2_params::Argon2Params;

mod unlocked_master_key;
pub use unlocked_master_key::UnlockedMasterKey;

mod create;
mod layout;
mod unlock;

use layout::Layout;

#[cfg(test)]
mod tests;

/// A Master Key protected by a Passphrase, as the bytes a device stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMasterKey {
    bytes: Vec<u8>,
    layout: Layout,
}

/// Offsets of the fixed part of the form.
mod offset {
    pub(super) const VERSION: usize = 5;
    pub(super) const RESERVED: usize = 6;
    pub(super) const SALT_LEN: usize = 7;
    pub(super) const MEMORY_KIB: std::ops::Range<usize> = 8..12;
    pub(super) const ITERATIONS: std::ops::Range<usize> = 12..16;
    pub(super) const PARALLELISM: std::ops::Range<usize> = 16..20;
    /// Where the salt starts, and therefore how long the fixed part is.
    pub(super) const SALT: usize = 20;
}

/// The plaintext this form encrypts: the key, then its epoch as 8 big-endian bytes.
const PLAINTEXT_LEN: usize = MasterKey::BYTE_LEN + 8;

impl StoredMasterKey {
    /// Length of the magic in bytes.
    pub const MAGIC_LEN: usize = 5;

    /// The bytes a stored Master Key starts with.
    pub const MAGIC: [u8; Self::MAGIC_LEN] = *b"CFMK1";

    /// The version this crate writes and reads.
    pub const VERSION: u8 = 0x01;

    /// Length of the salt this build draws, in bytes.
    pub const SALT_LEN: usize = 16;

    /// Takes stored bytes, checking that they are this form at all.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        let layout = Layout::parse(&bytes)?;
        Ok(Self { bytes, layout })
    }

    /// The bytes to store.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Takes the bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// The Argon2id cost these bytes were written at.
    pub fn params(&self) -> Argon2Params {
        self.layout.params
    }
}
