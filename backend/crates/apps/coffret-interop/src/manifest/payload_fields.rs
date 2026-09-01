//! What a manifest states about a control object's payload (FM-15, FM-16,
//! FM-17).
//!
//! A fixture's control payload is produced by `coffret-format`'s encoder, and
//! the expectation it is checked against cannot come from the same place — an
//! expectation read back out of the object under test would agree with any bug
//! that object carries. So the field names, the nesting, and the orders are
//! written out again here, from the domain values the fixture was built from,
//! and nothing in this module consults the encoder.
//!
//! The values themselves are the fixture's own content rather than anything the
//! format derives, which is why the manifest may state them at all: a Container
//! ID, an `mtime`, a slot token are what the fixture put in, not what encoding
//! it computed.

use coffret_format::IndexSnapshotPayload;
use coffret_model::{
    ContainerAddition, ContainerId, ContainerKeyStatus, ContainerKind, ContainerSummary,
    EntryLocation, EntryMetadata, JournalRecord, KeyringEntry, KeyringMapping,
};

use super::{BodyField, BodyValue};

/// The fields FM-15 gives the payload of `record`.
pub fn journal_record_fields(record: &JournalRecord) -> Vec<BodyField> {
    let mut additions: Vec<&ContainerAddition> = record.additions.iter().collect();
    additions.sort_by_key(|addition| addition.container.id);
    let mut removals: Vec<&ContainerId> = record.removals.iter().collect();
    removals.sort();

    let mut fields = vec![BodyField::uint("schema", 1)];
    if let Some(prev) = record.prev {
        fields.push(BodyField::uint("prev", prev.get()));
    }
    if let Some(slot) = &record.next_commit_slot {
        fields.push(BodyField::text("next_commit_slot", slot));
    }
    if let Some(slot) = &record.snapshot_slot {
        fields.push(BodyField::text("snapshot_slot", slot));
    }
    fields.push(BodyField::uint(
        "keyring_generation",
        record.keyring.generation().get(),
    ));
    fields.push(BodyField::uint(
        "keyring_replica_count",
        u64::from(record.keyring.replica_count()),
    ));
    fields.push(BodyField::text(
        "keyring_set_digest",
        record.keyring.set_digest(),
    ));
    fields.push(BodyField::array(
        "additions",
        additions
            .iter()
            .map(|addition| {
                let mut map = container_fields(&addition.container);
                map.push(BodyField::array(
                    "entries",
                    addition
                        .entries
                        .iter()
                        .map(|entry| BodyValue::Map {
                            value: entry_fields(entry),
                        })
                        .collect(),
                ));
                BodyValue::Map { value: map }
            })
            .collect(),
    ));
    fields.push(BodyField::array(
        "removals",
        removals
            .iter()
            .map(|id| BodyValue::Bytes {
                value: crate::hex::encode(id.as_bytes()),
            })
            .collect(),
    ));
    fields
}

/// The fields FM-16 gives the payload of `snapshot`.
pub fn index_snapshot_fields(snapshot: &IndexSnapshotPayload) -> Vec<BodyField> {
    let checkpoint = &snapshot.content.checkpoint;
    let mut containers: Vec<&ContainerSummary> = snapshot.content.containers.iter().collect();
    containers.sort_by_key(|container| container.id);
    let mut entries: Vec<&EntryLocation> = snapshot.content.entries.iter().collect();
    entries.sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));

    let mut fields = vec![
        BodyField::uint("schema", 1),
        BodyField::uint("head_generation", checkpoint.head_generation.get()),
        BodyField::uint("journal_generation", checkpoint.journal_generation.get()),
    ];
    if let Some(slot) = &checkpoint.next_commit_slot {
        fields.push(BodyField::text("next_commit_slot", slot));
    }
    fields.push(BodyField::uint(
        "keyring_generation",
        checkpoint.keyring.generation().get(),
    ));
    fields.push(BodyField::uint(
        "keyring_replica_count",
        u64::from(checkpoint.keyring.replica_count()),
    ));
    fields.push(BodyField::text(
        "keyring_set_digest",
        checkpoint.keyring.set_digest(),
    ));
    fields.push(BodyField::array(
        "containers",
        containers
            .iter()
            .map(|container| BodyValue::Map {
                value: container_fields(container),
            })
            .collect(),
    ));
    fields.push(BodyField::array(
        "entries",
        entries
            .iter()
            .map(|location| {
                let mut map = entry_fields(&location.entry);
                let position = containers
                    .iter()
                    .position(|container| container.id == location.container_id)
                    .expect("a fixture Entry is held by a Container the fixture lists");
                map.push(BodyField::uint("container", position as u64));
                BodyValue::Map { value: map }
            })
            .collect(),
    ));

    // The two fields an activation Snapshot carries and an ordinary one may not
    // (MR-2). `adopted_from` has no field here at all, whichever kind this is:
    // a Snapshot carries no device state (CK-7).
    if let Some(activation) = &snapshot.activation {
        fields.push(BodyField::uint(
            "base_head_generation",
            activation.base_head_generation.get(),
        ));
        if let Some(slot) = &activation.activation_slot {
            fields.push(BodyField::text("activation_slot", slot));
        }
    }
    fields
}

/// The fields FM-17 gives the payload of `mapping`.
///
/// The `set_digest` is not among them, and could not be: it is taken over this
/// array, so a manifest stating it as a field would state something no payload
/// carries. Where it *is* stated is the replica's object name, which the
/// exchange compares against the digest each side computes from the mapping it
/// decoded.
pub fn keyring_fields(mapping: &KeyringMapping) -> Vec<BodyField> {
    let mut entries: Vec<&KeyringEntry> = mapping.entries.iter().collect();
    entries.sort_by_key(|entry| entry.container_id);

    vec![
        BodyField::uint("schema", 1),
        BodyField::array(
            "mapping",
            entries
                .iter()
                .map(|entry| BodyValue::Map {
                    value: vec![
                        BodyField::bytes("id", entry.container_id.as_bytes()),
                        match entry.key {
                            ContainerKeyStatus::Envelope(envelope) => {
                                BodyField::bytes("envelope", envelope.as_bytes())
                            }
                            ContainerKeyStatus::KeyLost => BodyField::bool("key_lost", true),
                        },
                    ],
                })
                .collect(),
        ),
    ]
}

/// The five fields a Container is recorded with, shared by both schemas.
fn container_fields(container: &ContainerSummary) -> Vec<BodyField> {
    let mut fields = vec![
        BodyField::bytes("id", container.id.as_bytes()),
        BodyField::text(
            "kind",
            match container.kind {
                ContainerKind::OneFile => "one-file",
                ContainerKind::Pack => "pack",
            },
        ),
        BodyField::bytes("ciphertext_hash", container.ciphertext_hash.as_bytes()),
        BodyField::uint("ciphertext_len", container.ciphertext_len),
    ];
    if let Some(object_ref) = &container.object_ref {
        fields.push(BodyField::text("object_ref", object_ref.as_str()));
    }
    fields
}

/// The entry map in the catalog's spelling, which both payload schemas carry
/// (FM-15, FM-16).
///
/// The same values a Container's own meta section records, under the keys a
/// record and a Snapshot give them: `path`, `mtime`, and an optional `btime`
/// rather than FM-9's `original_` names. `derived_from` is the one place the
/// prefix survives, because it names an Entry inside an object already written
/// and no rename reaches in there.
fn entry_fields(entry: &EntryMetadata) -> Vec<BodyField> {
    let mut fields = vec![
        BodyField::text("path", entry.path.as_str()),
        BodyField::uint("offset", entry.offset),
        BodyField::uint("size", entry.size),
        BodyField::int("mtime", entry.mtime.as_unix_seconds()),
    ];
    if let Some(btime) = entry.btime {
        fields.push(BodyField::int("btime", btime.as_unix_seconds()));
    }
    fields.push(BodyField::bytes("hash", entry.hash.as_bytes()));
    if let Some(derived_from) = &entry.derived_from {
        fields.push(BodyField::map(
            "derived_from",
            vec![
                BodyField::bytes("container_id", derived_from.container_id.as_bytes()),
                BodyField::text("original_path", derived_from.path.as_str()),
            ],
        ));
    }
    if let Some(mime) = &entry.mime {
        fields.push(BodyField::text("mime", mime));
    }
    fields
}
