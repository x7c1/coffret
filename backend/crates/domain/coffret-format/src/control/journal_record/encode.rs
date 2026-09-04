use ciborium::Value;
use coffret_model::{ContainerAddition, JournalRecord};

use super::{
    ADDITIONS, ENTRIES, KEYRING_GENERATION, KEYRING_REPLICA_COUNT, KEYRING_SET_DIGEST,
    NEXT_COMMIT_SLOT, PREV, REMOVALS, SCHEMA, SNAPSHOT_SLOT,
};
use crate::control::cbor::{serialization_failed, write_body, MapBuilder, SCHEMA_FIELD};
use crate::control::wire_catalog_entry::WireCatalogEntry;
use crate::control::{wire_container, ControlPayload};
use crate::error::Result;

/// Serializes a Journal record to the payload a control object carries (FM-15).
///
/// The epoch comes off the record itself, so the payload the framing seals and
/// the record it was made from cannot name two different Master Keys (FM-13).
/// The record's generation is the header's and appears nowhere in here.
///
/// `additions` and `removals` are written in the order the record holds them,
/// which is the Container ID order FM-15 fixes: putting them in it is
/// [`JournalRecord`]'s own business, and a writer whose collections arrive in
/// spool order sorts through its `canonical` rather than here. Sorting again
/// here would make this a second statement of that rule.
pub fn encode(record: &JournalRecord) -> Result<ControlPayload> {
    let mut map = MapBuilder::new();
    map.uint(SCHEMA_FIELD, SCHEMA)
        .optional_uint(PREV, record.prev().map(|generation| generation.get()))
        .optional_text(NEXT_COMMIT_SLOT, record.next_commit_slot())
        .optional_text(SNAPSHOT_SLOT, record.snapshot_slot())
        .uint(KEYRING_GENERATION, record.keyring().generation().get())
        .uint(
            KEYRING_REPLICA_COUNT,
            u64::from(record.keyring().replica_count()),
        )
        .text(KEYRING_SET_DIGEST, record.keyring().set_digest())
        .value(
            ADDITIONS,
            Value::Array(
                record
                    .additions()
                    .iter()
                    .map(addition_value)
                    .collect::<Result<Vec<_>>>()?,
            ),
        )
        .value(
            REMOVALS,
            Value::Array(
                record
                    .removals()
                    .iter()
                    .map(|id| Value::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
        );

    Ok(ControlPayload::new(
        record.master_key_epoch(),
        write_body(&map.build())?,
    ))
}

/// One addition: the Container's five fields, then its entry table (CP-11).
///
/// The entry table keeps the order the Container's own meta section gives it,
/// which is the plaintext stream order FM-9 fixes — a copy of that table is
/// exactly what the record carries, so re-ordering it here would make the copy
/// disagree with the original.
fn addition_value(addition: &ContainerAddition) -> Result<Value> {
    let mut map = wire_container::to_map(addition.container());
    let entries = addition
        .entries()
        .iter()
        .map(|entry| {
            Value::serialized(&WireCatalogEntry::from(entry)).map_err(serialization_failed)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(map.value(ENTRIES, Value::Array(entries)).build())
}
