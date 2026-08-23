use ciborium::Value;
use coffret_model::{
    ContainerAddition, ContainerId, EntryMetadata, Generation, JournalRecord, KeyringCommitment,
};

use super::{
    ADDITIONS, ENTRIES, KEYRING_GENERATION, KEYRING_REPLICA_COUNT, KEYRING_SET_DIGEST,
    NEXT_COMMIT_SLOT, PREV, REMOVALS, SCHEMA, SNAPSHOT_SLOT,
};
use crate::control::canonical_order::require_strictly_increasing;
use crate::control::cbor::{read_body, Fields, SCHEMA_FIELD};
use crate::control::{wire_container, ControlPayload};
use crate::error::{Error, Result};
use crate::meta::WireEntry;

/// Parses a Journal record out of the payload a control object carried (FM-15).
///
/// The generation is the one the object's own header declared: a record does
/// not repeat it, so the caller passes what the framing authenticated (FM-11).
/// The epoch comes off the payload, where FM-13 puts it for every kind.
///
/// The array orders are verified rather than restored, for the reason FM-15
/// gives.
pub fn decode(payload: &ControlPayload, generation: Generation) -> Result<JournalRecord> {
    let value = read_body(&payload.body, malformed)?;
    let fields = Fields::of(&value, malformed)?;

    let schema = fields.uint(SCHEMA_FIELD)?;
    if schema < SCHEMA {
        return Err(Error::UnsupportedJournalRecordSchema { schema });
    }

    let additions = fields
        .array(ADDITIONS)?
        .iter()
        .map(|value| addition(&fields.map(value)?))
        .collect::<Result<Vec<_>>>()?;
    require_strictly_increasing(ADDITIONS, &additions, |left, right| {
        left.container.id.cmp(&right.container.id)
    })?;

    let removals = fields
        .array(REMOVALS)?
        .iter()
        .map(container_id)
        .collect::<Result<Vec<_>>>()?;
    require_strictly_increasing(REMOVALS, &removals, Ord::cmp)?;

    Ok(JournalRecord {
        generation,
        prev: fields.optional_uint(PREV)?.map(Generation::new),
        master_key_epoch: payload.master_key_epoch,
        keyring: KeyringCommitment::new(
            Generation::new(fields.uint(KEYRING_GENERATION)?),
            fields.u16(KEYRING_REPLICA_COUNT)?,
            &fields.text(KEYRING_SET_DIGEST)?,
        )?,
        next_commit_slot: fields.optional_text(NEXT_COMMIT_SLOT)?,
        snapshot_slot: fields.optional_text(SNAPSHOT_SLOT)?,
        additions,
        removals,
    })
}

/// One addition: the Container's five fields, plus the entry table beside them.
fn addition(fields: &Fields<'_>) -> Result<ContainerAddition> {
    Ok(ContainerAddition {
        container: wire_container::from_fields(fields, malformed)?,
        entries: fields
            .array(ENTRIES)?
            .iter()
            .map(entry)
            .collect::<Result<Vec<_>>>()?,
    })
}

/// One element of an entry table, read as exactly FM-9's entry map.
fn entry(value: &Value) -> Result<EntryMetadata> {
    value
        .deserialized::<WireEntry>()
        .map_err(|error| malformed(error.to_string()))?
        .to_metadata()
}

fn container_id(value: &Value) -> Result<ContainerId> {
    match value {
        Value::Bytes(bytes) => Ok(ContainerId::from_slice(bytes)?),
        other => Err(malformed(format!(
            "a removal is a byte string, found {}",
            crate::control::cbor::describe(other)
        ))),
    }
}

/// What a field of the wrong shape in this schema is reported as.
fn malformed(detail: String) -> Error {
    Error::MalformedJournalRecord { detail }
}
