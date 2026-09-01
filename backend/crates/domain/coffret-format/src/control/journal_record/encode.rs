use ciborium::Value;
use coffret_model::{ContainerAddition, ContainerId, JournalRecord};

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
/// Ordering `additions` and `removals` by Container ID happens here, not at the
/// caller, whatever order a writer held them in.
pub fn encode(record: &JournalRecord) -> Result<ControlPayload> {
    let mut additions: Vec<&ContainerAddition> = record.additions.iter().collect();
    additions.sort_by_key(|addition| addition.container.id);
    let mut removals: Vec<&ContainerId> = record.removals.iter().collect();
    removals.sort();

    let mut map = MapBuilder::new();
    map.uint(SCHEMA_FIELD, SCHEMA)
        .optional_uint(PREV, record.prev.map(|generation| generation.get()))
        .optional_text(NEXT_COMMIT_SLOT, record.next_commit_slot.as_deref())
        .optional_text(SNAPSHOT_SLOT, record.snapshot_slot.as_deref())
        .uint(KEYRING_GENERATION, record.keyring.generation().get())
        .uint(
            KEYRING_REPLICA_COUNT,
            u64::from(record.keyring.replica_count()),
        )
        .text(KEYRING_SET_DIGEST, record.keyring.set_digest())
        .value(
            ADDITIONS,
            Value::Array(
                additions
                    .iter()
                    .map(|addition| addition_value(addition))
                    .collect::<Result<Vec<_>>>()?,
            ),
        )
        .value(
            REMOVALS,
            Value::Array(
                removals
                    .iter()
                    .map(|id| Value::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
        );

    Ok(ControlPayload::new(
        record.master_key_epoch,
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
    let mut map = wire_container::to_map(&addition.container);
    let entries = addition
        .entries
        .iter()
        .map(|entry| {
            Value::serialized(&WireCatalogEntry::from(entry)).map_err(serialization_failed)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(map.value(ENTRIES, Value::Array(entries)).build())
}
