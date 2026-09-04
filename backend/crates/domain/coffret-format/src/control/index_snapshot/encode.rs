use std::collections::BTreeMap;

use ciborium::Value;
use coffret_model::{ContainerId, EntryLocation};

use super::{
    IndexSnapshotPayload, ACTIVATION_SLOT, BASE_HEAD_GENERATION, CONTAINER, CONTAINERS, ENTRIES,
    HEAD_GENERATION, JOURNAL_GENERATION, KEYRING_GENERATION, KEYRING_REPLICA_COUNT,
    KEYRING_SET_DIGEST, NEXT_COMMIT_SLOT, SCHEMA,
};
use crate::control::cbor::{serialization_failed, write_body, MapBuilder, SCHEMA_FIELD};
use crate::control::wire_catalog_entry::WireCatalogEntry;
use crate::control::{wire_container, ControlPayload};
use crate::error::{Error, Result};

/// Serializes an Index Snapshot to the payload a control object carries
/// (FM-16).
///
/// The epoch comes off the checkpoint, so the payload the framing seals and the
/// content it was made from cannot name two different Master Keys (CK-3,
/// FM-13).
///
/// `containers` and `entries` are written in the order the content holds them,
/// which is the Container ID and Entry Path order FM-16 fixes: putting them in
/// it is [`SnapshotContent`](coffret_model::SnapshotContent)'s own business,
/// and an Index that reported them in some other order sorts through its
/// `canonical`. Sorting again here would make this a second statement of that
/// rule.
pub fn encode(payload: &IndexSnapshotPayload) -> Result<ControlPayload> {
    let content = &payload.content;
    let checkpoint = content.checkpoint();

    let positions: BTreeMap<ContainerId, u64> = content
        .containers()
        .iter()
        .enumerate()
        .map(|(position, container)| (container.id, position as u64))
        .collect();

    let mut map = MapBuilder::new();
    map.uint(SCHEMA_FIELD, SCHEMA)
        .uint(HEAD_GENERATION, checkpoint.head_generation().get())
        .uint(JOURNAL_GENERATION, checkpoint.journal_generation().get())
        .optional_text(NEXT_COMMIT_SLOT, checkpoint.next_commit_slot())
        .uint(KEYRING_GENERATION, checkpoint.keyring().generation().get())
        .uint(
            KEYRING_REPLICA_COUNT,
            u64::from(checkpoint.keyring().replica_count()),
        )
        .text(KEYRING_SET_DIGEST, checkpoint.keyring().set_digest())
        .value(
            CONTAINERS,
            Value::Array(
                content
                    .containers()
                    .iter()
                    .map(|container| wire_container::to_map(container).build())
                    .collect(),
            ),
        )
        .value(
            ENTRIES,
            Value::Array(
                content
                    .entries()
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
        checkpoint.master_key_epoch(),
        write_body(&map.build())?,
    ))
}

/// One Entry: the catalog's entry map, plus the index of its Container (FM-16).
fn entry_value(
    index: usize,
    location: &EntryLocation,
    positions: &BTreeMap<ContainerId, u64>,
) -> Result<Value> {
    // Every Entry names a Container the content lists, which
    // `SnapshotContent::new` held when the value was built (FM-16), so nothing
    // here states that rule a second time. What is left is a lookup that has to
    // answer something, and the rule it leans on belongs to another crate: an
    // encoder a server calls reports a value it cannot write rather than taking
    // the process down over it.
    let Some(container) = positions.get(&location.container_id).copied() else {
        return Err(Error::SnapshotEntryWithoutContainer {
            entry: index,
            container_id: location.container_id,
        });
    };
    let mut fields = match Value::serialized(&WireCatalogEntry::from(&location.entry))
        .map_err(serialization_failed)?
    {
        Value::Map(fields) => fields,
        // `WireCatalogEntry` is a struct, so serde has only one shape to write it in.
        other => unreachable!("an entry map serialized to {other:?}"),
    };
    fields.push((Value::Text(CONTAINER.to_owned()), Value::from(container)));
    Ok(Value::Map(fields))
}
