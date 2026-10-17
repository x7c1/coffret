use ciborium::Value;
use coffret_model::{
    ContainerAddition, ContainerId, EntryMetadata, Generation, JournalRecord, KeyringCommitment,
};

use super::rename;
use super::{
    ADDITIONS, ENTRIES, KEYRING_GENERATION, KEYRING_REPLICA_COUNT, KEYRING_SET_DIGEST, MIN_SCHEMA,
    NEXT_COMMIT_SLOT, PREV, REMOVALS, RENAMES, RENAMES_SCHEMA, REQUIRED_SCHEMA, SCHEMA,
    SNAPSHOT_SLOT,
};
use crate::control::cbor::{deserialization_failed, read_body, Fields, SCHEMA_FIELD};
use crate::control::wire_catalog_entry::WireCatalogEntry;
use crate::control::{wire_container, ControlPayload};
use crate::error::{Error, MalformedDetail, Result};
use crate::field_numbers;

/// Parses a Journal record out of the payload a control object carried
/// (spec: FM-15).
///
/// The generation is the one the object's own header declared: a record does
/// not repeat it, so the caller passes what the framing authenticated
/// (spec: FM-11). The epoch comes off the payload, where FM-13 puts it for
/// every kind.
///
/// `prev` is the record's own statement of the head it was built on, and it is
/// held against that authenticated generation here, so a replay follows the
/// chain out of the payload rather than out of the name the object was fetched
/// under (spec: FM-15).
///
/// The array orders are verified rather than restored, for the reason FM-15
/// gives. Of the rules FM-15 gives `renames`, this holds the ones the record
/// alone shows — the order, a path that is an Entry Path outside the reserved
/// derived namespace — and leaves the ones that need the catalog the record is
/// applied to to the Index that applies it.
///
/// `required_schema` is read before anything else the record holds, and a
/// record naming a schema above the one this build implements is refused
/// there with [`Error::JournalRecordNeedsNewerBuild`]: never skipped, and never
/// applied in part with the fields this build does not know ignored
/// (spec: FM-9, FM-15). A record that carries `renames` without naming the
/// schema they need is malformed, since every writer of them names it.
pub fn decode(payload: &ControlPayload, generation: Generation) -> Result<JournalRecord> {
    let value = read_body(
        &payload.body,
        malformed_detail,
        &field_numbers::JOURNAL_RECORD,
    )?;
    let fields = Fields::of(&value, malformed)?;

    let schema = fields.uint(SCHEMA_FIELD)?;
    if schema < MIN_SCHEMA {
        return Err(Error::UnsupportedJournalRecordSchema { schema });
    }
    let required = required_schema(&fields, generation)?;

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

    // Absent on a record that renames nothing, which every record of schema 2
    // is (spec: FM-15).
    let renames = match fields.get(RENAMES) {
        None => Vec::new(),
        Some(_) => fields
            .array(RENAMES)?
            .iter()
            .enumerate()
            .map(|(index, value)| rename::from_fields(index, &fields.map(value)?, malformed))
            .collect::<Result<Vec<_>>>()?,
    };
    if !renames.is_empty() && required < RENAMES_SCHEMA {
        return Err(malformed(format!(
            "a record carrying renames names {RENAMES_SCHEMA} or above as its required_schema, \
             found {}",
            match required {
                MIN_SCHEMA => "none".to_owned(),
                other => other.to_string(),
            }
        )));
    }

    JournalRecord::new(
        generation,
        fields.optional_generation(PREV)?,
        payload.master_key_epoch,
        KeyringCommitment::new(
            fields.generation(KEYRING_GENERATION)?,
            fields.u16(KEYRING_REPLICA_COUNT)?,
            &fields.text(KEYRING_SET_DIGEST)?,
        )?,
        fields.optional_text(NEXT_COMMIT_SLOT)?,
        fields.optional_text(SNAPSHOT_SLOT)?,
        additions,
        removals,
    )
    .and_then(|record| record.with_renames(renames))
    .map_err(refused_record)
}

/// The schema the record says its reader must implement, held against the one
/// this build does (spec: FM-9, FM-15).
///
/// Absent is the reader floor, [`MIN_SCHEMA`]. Present, it names something
/// above that floor: a value at or below it says what its absence already
/// says, and a second spelling of one state is refused as every other one is.
fn required_schema(fields: &Fields<'_>, generation: Generation) -> Result<u64> {
    match fields.optional_uint(REQUIRED_SCHEMA)? {
        None => Ok(MIN_SCHEMA),
        Some(required) if required > SCHEMA => Err(Error::JournalRecordNeedsNewerBuild {
            generation,
            required,
            implemented: SCHEMA,
        }),
        Some(required) if required <= MIN_SCHEMA => Err(malformed(format!(
            "required_schema is above the reader floor of {MIN_SCHEMA} where present, \
             found {required}"
        ))),
        Some(required) => Ok(required),
    }
}

/// The record's own refusal, in this crate's vocabulary (spec: FM-15).
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
/// here (spec: FM-9, FM-10).
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

/// One element of an entry table, read in the catalog's spelling (spec: FM-15).
fn entry(value: &Value) -> Result<EntryMetadata> {
    value
        .deserialized::<WireCatalogEntry>()
        .map_err(|error| deserialization_failed(error, malformed_detail))?
        .to_metadata(malformed)
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
    malformed_detail(MalformedDetail::Written(detail))
}

/// The same variant over either provenance of its detail, for the readers that
/// decode as well as inspect.
fn malformed_detail(detail: MalformedDetail) -> Error {
    Error::MalformedJournalRecord { detail }
}
