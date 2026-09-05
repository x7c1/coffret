use ciborium::Value;
use coffret_model::{
    ContainerSummary, ControlObjectKind, EntryLocation, Generation, IndexCheckpoint,
    KeyringCommitment, SnapshotContent,
};

use super::{
    IndexSnapshotPayload, SnapshotActivation, ACTIVATION_SLOT, BASE_HEAD_GENERATION, CONTAINER,
    CONTAINERS, ENTRIES, HEAD_GENERATION, JOURNAL_GENERATION, KEYRING_GENERATION,
    KEYRING_REPLICA_COUNT, KEYRING_SET_DIGEST, NEXT_COMMIT_SLOT, SCHEMA,
};
use crate::control::cbor::{read_body, Fields, SCHEMA_FIELD};
use crate::control::wire_catalog_entry::WireCatalogEntry;
use crate::control::{wire_container, ControlPayload};
use crate::error::{Error, Result};

/// Parses an Index Snapshot out of the payload a control object carried
/// (FM-16).
///
/// `kind` is what the object's authenticated header declared, and it decides
/// which payload this may be: the activation fields belong to `0x04` alone, so
/// an ordinary Snapshot carrying them and an activation Snapshot without them
/// are both rejected. That is the whole of the cross-check between the header
/// and the payload, and it is why a misfiled Snapshot cannot be read as the
/// other kind.
///
/// `generation` is the one the object's own name declared, and this is the one
/// place the name is held against the payload — symmetrical with
/// [`decode_journal_record`](super::super::decode_journal_record), which is
/// told the same thing. A Snapshot checkpoints the head it is named for,
/// whichever kind it is: an ordinary Snapshot at `idx-<generation>` is that
/// head's checkpoint (CK-10), and an activation Snapshot at
/// `head-<generation>` occupies that head position itself, because a Library
/// has one head chain across an epoch boundary (FM-13). So both are refused
/// where the payload's `head_generation` says otherwise, and no caller checks
/// it again.
///
/// What an activation carries beyond that is a base head strictly earlier than
/// the one it takes (FM-16). Comparing its `activation_slot` with that head's
/// `next_commit_slot` is not done here: FM-16 leaves that to the caller, which
/// is the only party holding the base head's record.
///
/// The array orders and every `container` index are verified rather than
/// repaired, for the reason FM-16 gives.
pub fn decode(
    payload: &ControlPayload,
    kind: ControlObjectKind,
    generation: Generation,
) -> Result<IndexSnapshotPayload> {
    let activating = match kind {
        ControlObjectKind::IndexSnapshot => false,
        ControlObjectKind::ActivationSnapshot => true,
        other => return Err(Error::NotAnIndexSnapshotKind { kind: other }),
    };

    let value = read_body(&payload.body, malformed)?;
    let fields = Fields::of(&value, malformed)?;

    let schema = fields.uint(SCHEMA_FIELD)?;
    if schema < SCHEMA {
        return Err(Error::UnsupportedIndexSnapshotSchema { schema });
    }

    let containers = fields
        .array(CONTAINERS)?
        .iter()
        .map(|value| wire_container::from_fields(&fields.map(value)?, malformed))
        .collect::<Result<Vec<ContainerSummary>>>()?;

    let entries = fields
        .array(ENTRIES)?
        .iter()
        .enumerate()
        .map(|(index, value)| entry(index, value, &fields, &containers))
        .collect::<Result<Vec<_>>>()?;

    let head_generation = fields.generation(HEAD_GENERATION)?;
    if head_generation != generation {
        return Err(Error::SnapshotCheckpointsAnotherHead {
            generation,
            head_generation,
        });
    }
    let checkpoint = IndexCheckpoint::new(
        payload.master_key_epoch,
        head_generation,
        fields.generation(JOURNAL_GENERATION)?,
        fields.optional_text(NEXT_COMMIT_SLOT)?,
        KeyringCommitment::new(
            fields.generation(KEYRING_GENERATION)?,
            fields.u16(KEYRING_REPLICA_COUNT)?,
            &fields.text(KEYRING_SET_DIGEST)?,
        )?,
    )
    .map_err(refused_content)?;

    // A Snapshot carries no device state, so a decoded one says nothing about
    // which checkpoint an Index adopted (CK-7).
    let adopted_from = None;

    Ok(IndexSnapshotPayload {
        content: SnapshotContent::new(checkpoint, adopted_from, containers, entries)
            .map_err(refused_content)?,
        activation: activation(&fields, activating, head_generation)?,
    })
}

/// A Snapshot's own refusal, in this crate's vocabulary (CK-1, FM-16).
///
/// The order and the dangling Container already have names here, and a reader
/// that told them apart before keeps telling them apart.
fn refused_content(error: coffret_model::Error) -> Error {
    match error {
        coffret_model::Error::CollectionOutOfCanonicalOrder { collection, index } => {
            Error::ControlPayloadOutOfOrder {
                array: collection,
                index,
            }
        }
        coffret_model::Error::SnapshotEntryWithoutContainer {
            entry,
            container_id,
        } => Error::SnapshotEntryWithoutContainer {
            entry,
            container_id,
        },
        coffret_model::Error::CheckpointJournalAheadOfHead {
            head_generation,
            journal_generation,
        } => Error::CheckpointJournalAheadOfHead {
            head_generation,
            journal_generation,
        },
        other => Error::Model(other),
    }
}

/// The activation fields, checked against the kind the header declared and the
/// head this Snapshot takes.
fn activation(
    fields: &Fields<'_>,
    activating: bool,
    head_generation: Generation,
) -> Result<Option<SnapshotActivation>> {
    if !activating {
        for field in [BASE_HEAD_GENERATION, ACTIVATION_SLOT] {
            if fields.get(field).is_some() {
                return Err(Error::ActivationFieldOnOrdinarySnapshot { field });
            }
        }
        return Ok(None);
    }
    let base_head_generation = fields.optional_generation(BASE_HEAD_GENERATION)?.ok_or(
        Error::ActivationSnapshotFieldMissing {
            field: BASE_HEAD_GENERATION,
        },
    )?;
    if base_head_generation >= head_generation {
        return Err(Error::ActivationBaseHeadNotEarlier {
            head_generation,
            base_head_generation,
        });
    }
    Ok(Some(SnapshotActivation {
        base_head_generation,
        activation_slot: fields.optional_text(ACTIVATION_SLOT)?,
    }))
}

/// One Entry: the catalog's entry map, plus the Container it names by index
/// (FM-16).
fn entry(
    index: usize,
    value: &Value,
    outer: &Fields<'_>,
    containers: &[ContainerSummary],
) -> Result<EntryLocation> {
    let fields = outer.map(value)?;
    let container = fields.uint(CONTAINER)?;
    let position = usize::try_from(container)
        .ok()
        .filter(|position| *position < containers.len());
    let Some(position) = position else {
        return Err(Error::DanglingContainerIndex {
            entry: index,
            container,
            containers: containers.len(),
        });
    };
    Ok(EntryLocation {
        container_id: containers[position].id,
        // `container` is the Snapshot's own field and no part of the entry
        // map, and `WireCatalogEntry` ignores what it does not know, so the
        // whole map is handed over as it stands.
        entry: value
            .deserialized::<WireCatalogEntry>()
            .map_err(|error| malformed(error.to_string()))?
            .to_metadata()?,
    })
}

/// What a field of the wrong shape in this schema is reported as.
fn malformed(detail: String) -> Error {
    Error::MalformedIndexSnapshot { detail }
}
