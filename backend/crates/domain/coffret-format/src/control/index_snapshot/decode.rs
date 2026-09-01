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
use crate::control::canonical_order::require_strictly_increasing;
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
/// The array orders and every `container` index are verified rather than
/// repaired, for the reason FM-16 gives.
pub fn decode(payload: &ControlPayload, kind: ControlObjectKind) -> Result<IndexSnapshotPayload> {
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
    require_strictly_increasing(CONTAINERS, &containers, |left, right| {
        left.id.cmp(&right.id)
    })?;

    let entries = fields
        .array(ENTRIES)?
        .iter()
        .enumerate()
        .map(|(index, value)| entry(index, value, &fields, &containers))
        .collect::<Result<Vec<_>>>()?;
    require_strictly_increasing(ENTRIES, &entries, |left, right| {
        left.path().as_str().cmp(right.path().as_str())
    })?;

    Ok(IndexSnapshotPayload {
        content: SnapshotContent {
            checkpoint: IndexCheckpoint {
                master_key_epoch: payload.master_key_epoch,
                head_generation: Generation::new(fields.uint(HEAD_GENERATION)?),
                journal_generation: Generation::new(fields.uint(JOURNAL_GENERATION)?),
                next_commit_slot: fields.optional_text(NEXT_COMMIT_SLOT)?,
                keyring: KeyringCommitment::new(
                    Generation::new(fields.uint(KEYRING_GENERATION)?),
                    fields.u16(KEYRING_REPLICA_COUNT)?,
                    &fields.text(KEYRING_SET_DIGEST)?,
                )?,
            },
            // A Snapshot carries no device state, so a decoded one says nothing
            // about which checkpoint an Index adopted (CK-7).
            adopted_from: None,
            containers,
            entries,
        },
        activation: activation(&fields, activating)?,
    })
}

/// The activation fields, checked against the kind the header declared.
fn activation(fields: &Fields<'_>, activating: bool) -> Result<Option<SnapshotActivation>> {
    if !activating {
        for field in [BASE_HEAD_GENERATION, ACTIVATION_SLOT] {
            if fields.get(field).is_some() {
                return Err(Error::ActivationFieldOnOrdinarySnapshot { field });
            }
        }
        return Ok(None);
    }
    let base_head_generation = fields.optional_uint(BASE_HEAD_GENERATION)?.ok_or(
        Error::ActivationSnapshotFieldMissing {
            field: BASE_HEAD_GENERATION,
        },
    )?;
    Ok(Some(SnapshotActivation {
        base_head_generation: Generation::new(base_head_generation),
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
