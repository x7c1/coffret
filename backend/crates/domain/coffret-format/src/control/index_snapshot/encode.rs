use std::collections::BTreeMap;

use ciborium::Value;
use coffret_model::{ContainerId, EntryLocation};

use super::{
    IndexSnapshotPayload, ACTIVATION_SLOT, BASE_HEAD_GENERATION, CONTAINER, CONTAINERS, ENTRIES,
    HEAD_GENERATION, JOURNAL_GENERATION, KEYRING_GENERATION, KEYRING_REPLICA_COUNT,
    KEYRING_SET_DIGEST, NEXT_COMMIT_SLOT, SCHEMA,
};
use crate::control::cbor::{serialization_failed, write_body, MapBuilder, SCHEMA_FIELD};
use crate::control::{wire_container, ControlPayload};
use crate::error::{Error, Result};
use crate::meta::WireEntry;

/// Serializes an Index Snapshot to the payload a control object carries
/// (FM-16).
///
/// The epoch comes off the checkpoint, so the payload the framing seals and the
/// content it was made from cannot name two different Master Keys (CK-3,
/// FM-13).
///
/// Putting `containers` in Container ID order and `entries` in Entry Path order
/// happens here, whatever order the Index that produced this content reported
/// them in.
pub fn encode(payload: &IndexSnapshotPayload) -> Result<ControlPayload> {
    let content = &payload.content;
    let checkpoint = &content.checkpoint;

    let mut containers: Vec<&coffret_model::ContainerSummary> = content.containers.iter().collect();
    containers.sort_by_key(|container| container.id);
    let positions: BTreeMap<ContainerId, u64> = containers
        .iter()
        .enumerate()
        .map(|(position, container)| (container.id, position as u64))
        .collect();

    let mut entries: Vec<&EntryLocation> = content.entries.iter().collect();
    entries.sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));

    let mut map = MapBuilder::new();
    map.uint(SCHEMA_FIELD, SCHEMA)
        .uint(HEAD_GENERATION, checkpoint.head_generation.get())
        .uint(JOURNAL_GENERATION, checkpoint.journal_generation.get())
        .optional_text(NEXT_COMMIT_SLOT, checkpoint.next_commit_slot.as_deref())
        .uint(KEYRING_GENERATION, checkpoint.keyring.generation().get())
        .uint(
            KEYRING_REPLICA_COUNT,
            u64::from(checkpoint.keyring.replica_count()),
        )
        .text(KEYRING_SET_DIGEST, checkpoint.keyring.set_digest())
        .value(
            CONTAINERS,
            Value::Array(
                containers
                    .iter()
                    .map(|container| wire_container::to_map(container).build())
                    .collect(),
            ),
        )
        .value(
            ENTRIES,
            Value::Array(
                entries
                    .iter()
                    .enumerate()
                    .map(|(index, location)| entry_value(index, location, &positions))
                    .collect::<Result<Vec<_>>>()?,
            ),
        );

    if let Some(activation) = &payload.activation {
        map.uint(BASE_HEAD_GENERATION, activation.base_head_generation.get())
            .optional_text(ACTIVATION_SLOT, activation.activation_slot.as_deref());
    }

    // `adopted_from` is not written and has no field to be written into: which
    // checkpoint this Index adopted is the Index's own provenance, and a
    // Snapshot carries no device state (CK-7).
    Ok(ControlPayload::new(
        checkpoint.master_key_epoch,
        write_body(&map.build())?,
    ))
}

/// One Entry: exactly FM-9's entry map, plus the index of its Container.
fn entry_value(
    index: usize,
    location: &EntryLocation,
    positions: &BTreeMap<ContainerId, u64>,
) -> Result<Value> {
    let container =
        *positions
            .get(&location.container_id)
            .ok_or(Error::SnapshotEntryWithoutContainer {
                entry: index,
                container_id: location.container_id,
            })?;
    let mut fields =
        match Value::serialized(&WireEntry::from(&location.entry)).map_err(serialization_failed)? {
            Value::Map(fields) => fields,
            // `WireEntry` is a struct, so serde has only one shape to write it in.
            other => unreachable!("an entry map serialized to {other:?}"),
        };
    fields.push((Value::Text(CONTAINER.to_owned()), Value::from(container)));
    Ok(Value::Map(fields))
}
