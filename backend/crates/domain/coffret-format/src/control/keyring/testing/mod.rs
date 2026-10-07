//! Helpers shared by the Keyring payload's tests.

use coffret_model::{KeyEnvelope, KeyTable, KeyringElement, MasterKeyEpoch};

use crate::control::testing::{container_id, epoch};

/// The epoch every key table these helpers seal is written under.
pub(super) const EPOCH: u64 = 2;

pub(super) fn key_table_epoch() -> MasterKeyEpoch {
    epoch(EPOCH)
}

/// A key table holding both of the things a Keyring can hold.
///
/// Two Containers open through an envelope and one is recorded key-lost
/// (spec: KL-7), and the elements are handed over out of Container ID order on
/// purpose: what a case compares is then a key table holding them in the order
/// FM-17 fixes rather than in the order a caller happened to have them.
pub(super) fn key_table() -> KeyTable {
    key_table_of(vec![
        KeyringElement::envelope(container_id(0x40), envelope(0x40)),
        KeyringElement::key_lost(container_id(0x99)),
        KeyringElement::envelope(container_id(0x21), envelope(0x21)),
    ])
}

/// The key table `elements` spell, in the Container ID order FM-17 fixes whichever
/// order they arrive in.
pub(super) fn key_table_of(elements: Vec<KeyringElement>) -> KeyTable {
    KeyTable::canonical(elements)
        .expect("a fixture holds a key table that names each Container once")
}

/// The key table whose digest both implementations pin.
///
/// Deliberately smaller and duller than [`key_table`]: it exists so that the two
/// implementations state one expected digest each, in a shape that is easy to
/// spell identically in both languages. The TypeScript suite builds the same
/// two elements — `11…` with an envelope of `22` bytes, `33…` key-lost — and
/// asserts the same hex.
pub(super) fn pinned_key_table() -> KeyTable {
    key_table_of(vec![
        KeyringElement::envelope(container_id(0x11), envelope(0x22)),
        KeyringElement::key_lost(container_id(0x33)),
    ])
}

/// A Key Envelope whose seventy-two bytes are all `seed`.
///
/// The bytes are ciphertext to everything in this module: FM-17 carries an
/// envelope as an opaque byte string of the length FM-14 gives it, and whether
/// one unwraps is the Key Envelope's own business.
pub(super) fn envelope(seed: u8) -> KeyEnvelope {
    KeyEnvelope::from_bytes([seed; KeyEnvelope::BYTE_LEN])
}
