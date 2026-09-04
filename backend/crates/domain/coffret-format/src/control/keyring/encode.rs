use ciborium::Value;
use coffret_model::{ContainerKeyStatus, KeyringEntry, KeyringMapping, MasterKeyEpoch};

use super::{ENVELOPE, ID, KEY_LOST, MAPPING, SCHEMA};
use crate::control::cbor::{write_body, MapBuilder, SCHEMA_FIELD};
use crate::control::ControlPayload;
use crate::error::Result;

/// Serializes a Keyring mapping to the payload a replica carries (FM-17).
///
/// The epoch is handed in rather than taken off the mapping: which epoch a
/// generation belongs to is the Keyring's own numbering (KL-10) and not
/// something the mapping states, so the caller that knows which Master Key is
/// sealing this replica names it once, here (FM-13).
///
/// The same call produces every replica of a generation — the index and count
/// ride in the header (FM-11) — so a caller replicates by framing these bytes
/// R times rather than by encoding R payloads.
///
/// `mapping` is written in the order the mapping holds it, which is the
/// Container ID order FM-17 fixes: putting it in that order is
/// [`KeyringMapping`]'s own business, and a caller whose entries arrive in some
/// other order sorts through its `canonical`.
pub fn encode(
    mapping: &KeyringMapping,
    master_key_epoch: MasterKeyEpoch,
) -> Result<ControlPayload> {
    let mut map = MapBuilder::new();
    map.uint(SCHEMA_FIELD, SCHEMA)
        .value(MAPPING, mapping_value(mapping));

    Ok(ControlPayload::new(
        master_key_epoch,
        write_body(&map.build())?,
    ))
}

/// The `mapping` array, in the Container ID order FM-17 fixes.
///
/// [`set_digest()`](super::set_digest()) hashes exactly this value's encoding,
/// so this is the one array in the crate whose bytes are normative rather than
/// one valid CBOR spelling among several. What that costs is stated in
/// [`element`]; what it buys is that one mapping has one digest whichever
/// device wrote it (KL-1, KL-14).
pub(super) fn mapping_value(mapping: &KeyringMapping) -> Value {
    Value::Array(mapping.entries().iter().map(element).collect())
}

/// One element: the Container's ID, then the one thing the Keyring holds for it.
///
/// The two fields are written in that order deliberately. FM-17 hashes this
/// array as deterministic CBOR, whose map keys are ordered by their encoded
/// bytes: `id` is two characters and both `envelope` and `key_lost` are eight,
/// so `id` comes first. A writer that emitted the fields the other way round
/// would produce a payload every reader still accepts — the maps are read by
/// name — and a `set_digest` no other implementation computes.
fn element(entry: &KeyringEntry) -> Value {
    let mut map = MapBuilder::new();
    map.bytes(ID, entry.container_id.as_bytes());
    match entry.key {
        ContainerKeyStatus::Envelope(envelope) => {
            map.bytes(ENVELOPE, envelope.as_bytes());
        }
        // The marker's presence is what records the loss; FM-17 spells it
        // `true` so that one marker has one spelling.
        ContainerKeyStatus::KeyLost => {
            map.value(KEY_LOST, Value::Bool(true));
        }
    }
    map.build()
}
