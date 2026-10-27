use super::map_fields::MapFields;
use super::registered_field::{field, holding};

/// The field number `master_key_epoch` travels under in every control payload
/// map: the framing reads it before it knows which kind's map it holds
/// (spec: FM-13).
pub(crate) const MASTER_KEY_EPOCH_FIELD: u64 = 0;

/// The meta section's own map (spec: FM-9).
pub static META: MapFields = MapFields {
    map: "meta",
    fields: &[
        field("schema", 1),
        field("kind", 2),
        field("pad_len", 3),
        holding("entries", 4, &META_ENTRY),
    ],
};

/// One row of a meta section's entry table (spec: FM-9).
pub static META_ENTRY: MapFields = MapFields {
    map: "meta_entry",
    fields: &[
        field("captured_path", 0),
        field("offset", 1),
        field("size", 2),
        field("captured_mtime", 3),
        field("hash", 4),
        field("captured_btime", 5),
        field("mime", 6),
        holding("derived_from", 7, &DERIVED_FROM),
    ],
};

/// The parent reference an entry map carries, in whichever map (spec: FM-9).
pub static DERIVED_FROM: MapFields = MapFields {
    map: "derived_from",
    fields: &[field("container_id", 0), field("captured_path", 1)],
};

/// A Journal record's payload map (spec: FM-15).
pub static JOURNAL_RECORD: MapFields = MapFields {
    map: "journal_record",
    fields: &[
        field("master_key_epoch", MASTER_KEY_EPOCH_FIELD),
        field("schema", 1),
        field("prev", 2),
        field("next_commit_slot", 3),
        field("snapshot_slot", 4),
        field("keyring_generation", 5),
        field("keyring_replica_count", 6),
        field("keyring_set_digest", 7),
        holding("additions", 8, &ADDITION),
        field("removals", 9),
        holding("renames", 10, &RENAME),
        field("required_schema", 11),
        field("library_name", 12),
    ],
};

/// One element of a Journal record's `renames` (spec: FM-15).
pub static RENAME: MapFields = MapFields {
    map: "rename",
    fields: &[field("container", 0), field("entry", 1), field("path", 2)],
};

/// One element of a Journal record's `additions` (spec: FM-15).
pub static ADDITION: MapFields = MapFields {
    map: "addition",
    fields: &[
        field("id", 0),
        field("kind", 1),
        field("ciphertext_hash", 2),
        field("ciphertext_len", 3),
        field("object_ref", 4),
        holding("entries", 5, &ADDITION_ENTRY),
    ],
};

/// One element of an addition's entry table (spec: FM-15).
pub static ADDITION_ENTRY: MapFields = MapFields {
    map: "addition_entry",
    fields: &[
        field("path", 0),
        field("offset", 1),
        field("size", 2),
        field("mtime", 3),
        field("hash", 4),
        field("btime", 5),
        field("mime", 6),
        holding("derived_from", 7, &DERIVED_FROM),
        field("captured_path", 8),
        field("deposited_at", 9),
    ],
};

/// An Index Snapshot's payload map, ordinary and activation alike
/// (spec: FM-16).
pub static INDEX_SNAPSHOT: MapFields = MapFields {
    map: "index_snapshot",
    fields: &[
        field("master_key_epoch", MASTER_KEY_EPOCH_FIELD),
        field("schema", 1),
        field("head_generation", 2),
        field("next_commit_slot", 3),
        field("journal_generation", 4),
        field("keyring_generation", 5),
        field("keyring_replica_count", 6),
        field("keyring_set_digest", 7),
        holding("containers", 8, &SNAPSHOT_CONTAINER),
        holding("entries", 9, &SNAPSHOT_ENTRY),
        field("base_head_generation", 10),
        field("activation_slot", 11),
        field("library_name", 12),
    ],
};

/// One element of an Index Snapshot's `containers` (spec: FM-16).
pub static SNAPSHOT_CONTAINER: MapFields = MapFields {
    map: "snapshot_container",
    fields: &[
        field("id", 0),
        field("kind", 1),
        field("ciphertext_hash", 2),
        field("ciphertext_len", 3),
        field("object_ref", 4),
    ],
};

/// One element of an Index Snapshot's `entries` (spec: FM-16).
pub static SNAPSHOT_ENTRY: MapFields = MapFields {
    map: "snapshot_entry",
    fields: &[
        field("path", 0),
        field("offset", 1),
        field("size", 2),
        field("mtime", 3),
        field("hash", 4),
        field("btime", 5),
        field("mime", 6),
        holding("derived_from", 7, &DERIVED_FROM),
        field("captured_path", 8),
        field("container", 9),
        field("entry", 10),
        field("deposited_at", 11),
    ],
};

/// A Keyring replica's payload map (spec: FM-17).
pub static KEYRING: MapFields = MapFields {
    map: "keyring",
    fields: &[
        field("master_key_epoch", MASTER_KEY_EPOCH_FIELD),
        field("schema", 1),
        holding("mapping", 2, &KEYRING_ELEMENT),
    ],
};

/// One element of a Keyring's `mapping` (spec: FM-17).
pub static KEYRING_ELEMENT: MapFields = MapFields {
    map: "keyring_element",
    fields: &[field("id", 0), field("envelope", 1), field("key_lost", 2)],
};

/// Every map kind this crate declares, which is every map kind the registry
/// lists.
#[cfg(test)]
pub(crate) static ALL: [&MapFields; 12] = [
    &META,
    &META_ENTRY,
    &DERIVED_FROM,
    &JOURNAL_RECORD,
    &RENAME,
    &ADDITION,
    &ADDITION_ENTRY,
    &INDEX_SNAPSHOT,
    &SNAPSHOT_CONTAINER,
    &SNAPSHOT_ENTRY,
    &KEYRING,
    &KEYRING_ELEMENT,
];
