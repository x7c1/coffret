use ciborium::Value;
use coffret_model::{
    ContainerAddition, ContainerId, EntryMetadata, Generation, JournalRecord, KeyringCommitment,
};

use super::{
    ADDITIONS, ENTRIES, KEYRING_GENERATION, KEYRING_REPLICA_COUNT, KEYRING_SET_DIGEST,
    NEXT_COMMIT_SLOT, PREV, REMOVALS, SCHEMA, SNAPSHOT_SLOT,
};
use crate::control::cbor::{read_body, Fields, SCHEMA_FIELD};
use crate::control::wire_catalog_entry::WireCatalogEntry;
use crate::control::{wire_container, ControlPayload};
use crate::error::{Error, Result};

/// Parses a Journal record out of the payload a control object carried (FM-15).
///
/// The generation is the one the object's own header declared: a record does
/// not repeat it, so the caller passes what the framing authenticated (FM-11).
/// The epoch comes off the payload, where FM-13 puts it for every kind.
///
/// `prev` is the record's own statement of the head it was built on, and it is
/// held against that authenticated generation here, so a replay follows the
/// chain out of the payload rather than out of the name the object was fetched
/// under (FM-15).
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
        .enumerate()
        .map(|(index, value)| addition(index, &fields.map(value)?))
        .collect::<Result<Vec<_>>>()?;

    let removals = fields
        .array(REMOVALS)?
        .iter()
        .map(container_id)
        .collect::<Result<Vec<_>>>()?;

    JournalRecord::new(
        generation,
        fields.optional_uint(PREV)?.map(Generation::new),
        payload.master_key_epoch,
        KeyringCommitment::new(
            Generation::new(fields.uint(KEYRING_GENERATION)?),
            fields.u16(KEYRING_REPLICA_COUNT)?,
            &fields.text(KEYRING_SET_DIGEST)?,
        )?,
        fields.optional_text(NEXT_COMMIT_SLOT)?,
        fields.optional_text(SNAPSHOT_SLOT)?,
        additions,
        removals,
    )
    .map_err(refused_record)
}

/// The record's own refusal, in this crate's vocabulary (FM-15).
///
/// Two of the rules the constructor holds have had a name here since before it
/// did, and a reader that already tells `prev` apart from an array out of order
/// keeps telling them apart. The rest arrive as the model's refusal, which
/// names what it refused.
fn refused_record(error: coffret_model::Error) -> Error {
    match error {
        coffret_model::Error::JournalRecordPredecessorMismatch { generation, prev } => {
            Error::JournalRecordPrevMismatch { generation, prev }
        }
        coffret_model::Error::CollectionOutOfCanonicalOrder { collection, index } => {
            Error::ControlPayloadOutOfOrder {
                array: collection,
                index,
            }
        }
        other => Error::Model(other),
    }
}

/// One addition: the Container's five fields, plus the entry table beside them.
///
/// What makes the table an entry table — that it holds an Entry at all, and
/// that its Entries tile the Container's plaintext stream — is the aggregate's
/// own rule, so the values are handed to its constructor rather than checked
/// here (FM-9, FM-10).
fn addition(index: usize, fields: &Fields<'_>) -> Result<ContainerAddition> {
    let container = wire_container::from_fields(fields, malformed)?;
    let entries = fields
        .array(ENTRIES)?
        .iter()
        .map(entry)
        .collect::<Result<Vec<_>>>()?;

    ContainerAddition::new(container, entries).map_err(|error| refused_addition(index, error))
}

/// One addition's own refusal, placed in the array it came out of.
///
/// The aggregate names the Entry; which addition that Entry was in is what this
/// layer knows, and a reader looking at a payload needs both.
fn refused_addition(addition: usize, error: coffret_model::Error) -> Error {
    match error {
        coffret_model::Error::AdditionWithoutEntries => Error::AdditionWithoutEntries { addition },
        coffret_model::Error::AdditionEntriesDoNotTile {
            entry,
            expected,
            found,
        } => Error::AdditionEntriesDoNotTile {
            addition,
            entry,
            expected,
            found,
        },
        coffret_model::Error::AdditionNamesOnePathTwice { entry } => {
            Error::AdditionNamesOnePathTwice { addition, entry }
        }
        other => Error::Model(other),
    }
}

/// One element of an entry table, read in the catalog's spelling (FM-15).
fn entry(value: &Value) -> Result<EntryMetadata> {
    value
        .deserialized::<WireCatalogEntry>()
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
