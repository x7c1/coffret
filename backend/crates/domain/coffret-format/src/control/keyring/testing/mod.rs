//! Helpers shared by the Keyring payload's tests.

use coffret_model::{KeyEnvelope, KeyringEntry, KeyringMapping, MasterKeyEpoch};

use crate::control::testing::{container_id, epoch};

/// The epoch every mapping these helpers seal is written under.
pub(super) const EPOCH: u64 = 2;

pub(super) fn mapping_epoch() -> MasterKeyEpoch {
    epoch(EPOCH)
}

/// A mapping holding both of the things a Keyring can hold.
///
/// Two Containers open through an envelope and one is recorded key-lost
/// (KL-7), and the entries are handed over out of Container ID order on
/// purpose: a case comparing bytes is then comparing what the encoder ordered
/// rather than what a caller happened to hold (FM-17).
pub(super) fn mapping() -> KeyringMapping {
    KeyringMapping::new(vec![
        KeyringEntry::envelope(container_id(0x40), envelope(0x40)),
        KeyringEntry::key_lost(container_id(0x99)),
        KeyringEntry::envelope(container_id(0x21), envelope(0x21)),
    ])
}

/// The mapping whose digest both implementations pin.
///
/// Deliberately smaller and duller than [`mapping`]: it exists so that the two
/// implementations state one expected digest each, in a shape that is easy to
/// spell identically in both languages. The TypeScript suite builds the same
/// two entries — `11…` with an envelope of `22` bytes, `33…` key-lost — and
/// asserts the same hex.
pub(super) fn pinned_mapping() -> KeyringMapping {
    KeyringMapping::new(vec![
        KeyringEntry::envelope(container_id(0x11), envelope(0x22)),
        KeyringEntry::key_lost(container_id(0x33)),
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
