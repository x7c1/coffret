use std::path::PathBuf;

use coffret_model::{
    ContainerId, ContainerKind, ContainerSummary, ContentHash, ControlObjectName, DerivedFrom,
    EntryLocation, EntryMetadata, EntryPath, Generation, IndexCheckpoint, KeyringCommitment,
    MasterKeyEpoch, Mtime, ObjectRef,
};
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalEntry, LocalEntryState, LocalObservation, Mapping, PendingUpload,
    RootIdentity, SpoolState,
};
use coffret_usecase::IndexResult;
use rusqlite::Row;

use crate::error::{translate, unreadable, unreadable_model};

/// How a domain value is spelled in a column, and read back out of one.
///
/// SQLite integers are 64 bits and signed, while offsets, sizes, generations,
/// and epochs are unsigned. Rather than refusing the upper half of the range or
/// widening every column to text, a value is kept as the same 64 bits and read
/// back as the same 64 bits, which round-trips all of `u64` exactly. Nothing is
/// ever ordered by one of these columns — the orders the port promises are over
/// Container IDs and Entry Path bytes — so the sign the top bit would give a
/// value that large never decides anything.
pub(crate) const fn to_integer(value: u64) -> i64 {
    value.cast_signed()
}

/// The inverse of [`to_integer`].
pub(crate) const fn from_integer(value: i64) -> u64 {
    value.cast_unsigned()
}

/// How a Container's kind is spelled, in the meta section's own vocabulary
/// (spec: FM-9, PK-15).
pub(crate) const fn kind_text(kind: ContainerKind) -> &'static str {
    match kind {
        ContainerKind::OneFile => "one-file",
        ContainerKind::Pack => "pack",
    }
}

/// How this device's answer to "do I have this file" is spelled (spec: EP-10).
pub(crate) const fn state_text(state: LocalEntryState) -> &'static str {
    match state {
        LocalEntryState::Present => "present",
        LocalEntryState::Absent => "absent",
    }
}

/// How this device's answer to "is that spool file a whole Container" is spelled
/// (spec: OC-2).
pub(crate) const fn spool_state_text(state: SpoolState) -> &'static str {
    match state {
        SpoolState::Spooling => "spooling",
        SpoolState::Spooled => "spooled",
    }
}

/// One row of `containers`.
pub(crate) fn container_summary(row: &Row<'_>) -> IndexResult<ContainerSummary> {
    const OPERATION: &str = "reading a Container";
    Ok(ContainerSummary {
        id: container_id(row, "id", OPERATION)?,
        kind: match text(row, "kind", OPERATION)?.as_str() {
            "one-file" => ContainerKind::OneFile,
            "pack" => ContainerKind::Pack,
            found => return Err(unreadable(OPERATION, "Container kind", found)),
        },
        ciphertext_hash: content_hash(row, "ciphertext_hash", OPERATION)?,
        ciphertext_len: from_integer(integer(row, "ciphertext_len", OPERATION)?),
        object_ref: optional_text(row, "object_ref", OPERATION)?.map(ObjectRef::new),
    })
}

/// One row of `entries`.
pub(crate) fn entry_location(row: &Row<'_>) -> IndexResult<EntryLocation> {
    const OPERATION: &str = "reading an Entry";
    let derived_container = optional_blob(row, "derived_from_container", OPERATION)?;
    let derived_path = optional_text(row, "derived_from_path", OPERATION)?;
    let derived_from = match (derived_container, derived_path) {
        (Some(container), Some(path)) => Some(DerivedFrom {
            container_id: ContainerId::from_slice(&container)
                .map_err(unreadable_model(OPERATION))?,
            path: EntryPath::stored(path).map_err(unreadable_model(OPERATION))?,
        }),
        (None, None) => None,
        // Half a reference points at nothing, and guessing which half was meant
        // would attach derived data to the wrong parent. Which half is there
        // says whether it is the Container column or the path column that was
        // left unwritten; the path itself stays out of it.
        (Some(_), None) => {
            return Err(unreadable(
                OPERATION,
                "derived-from reference",
                "a Container with no Entry Path",
            ))
        }
        (None, Some(_)) => {
            return Err(unreadable(
                OPERATION,
                "derived-from reference",
                "an Entry Path with no Container",
            ))
        }
    };

    Ok(EntryLocation {
        container_id: container_id(row, "container_id", OPERATION)?,
        entry: EntryMetadata {
            path: entry_path(row, "path", OPERATION)?,
            offset: from_integer(integer(row, "offset", OPERATION)?),
            size: from_integer(integer(row, "size", OPERATION)?),
            mtime: Mtime::from_unix_seconds(integer(row, "mtime", OPERATION)?),
            hash: content_hash(row, "hash", OPERATION)?,
            derived_from,
            mime: optional_text(row, "mime", OPERATION)?,
        },
    })
}

/// The single row of `checkpoint`, and the checkpoint object it was adopted
/// from.
pub(crate) fn checkpoint(
    row: &Row<'_>,
) -> IndexResult<(IndexCheckpoint, Option<ControlObjectName>)> {
    const OPERATION: &str = "reading the checkpoint";
    let replica_count = integer(row, "keyring_replica_count", OPERATION)?;
    let checkpoint = IndexCheckpoint {
        master_key_epoch: MasterKeyEpoch::new(from_integer(integer(
            row,
            "master_key_epoch",
            OPERATION,
        )?))
        .map_err(unreadable_model(OPERATION))?,
        head_generation: Generation::new(from_integer(integer(row, "head_generation", OPERATION)?)),
        journal_generation: Generation::new(from_integer(integer(
            row,
            "journal_generation",
            OPERATION,
        )?)),
        next_commit_slot: optional_text(row, "next_commit_slot", OPERATION)?,
        keyring: KeyringCommitment::new(
            Generation::new(from_integer(integer(row, "keyring_generation", OPERATION)?)),
            u16::try_from(replica_count).map_err(|_| {
                unreadable(
                    OPERATION,
                    "Keyring replica count",
                    replica_count.to_string(),
                )
            })?,
            &text(row, "keyring_set_digest", OPERATION)?,
        )
        .map_err(unreadable_model(OPERATION))?,
    };
    let adopted_from = optional_text(row, "adopted_snapshot", OPERATION)?
        .map(|name| ControlObjectName::parse(&name))
        .transpose()
        .map_err(unreadable_model(OPERATION))?;

    Ok((checkpoint, adopted_from))
}

/// One row of `mappings`.
pub(crate) fn mapping(row: &Row<'_>) -> IndexResult<Mapping> {
    const OPERATION: &str = "reading a mapping";
    Ok(Mapping {
        prefix: optional_entry_path(row, "prefix", OPERATION)?,
        local_root: PathBuf::from(text(row, "local_root", OPERATION)?),
        root_identity: optional_text(row, "root_identity", OPERATION)?.map(RootIdentity::new),
    })
}

/// One row of `local_entries`.
pub(crate) fn local_entry(row: &Row<'_>) -> IndexResult<LocalEntry> {
    const OPERATION: &str = "reading a local file's row";
    Ok(LocalEntry {
        observation: LocalObservation {
            path: entry_path(row, "path", OPERATION)?,
            size: from_integer(integer(row, "observed_size", OPERATION)?),
            mtime: Mtime::from_unix_seconds(integer(row, "observed_mtime", OPERATION)?),
            at: DeviceTime::from_unix_seconds(integer(row, "observed_at", OPERATION)?),
        },
        state: match text(row, "state", OPERATION)?.as_str() {
            "present" => LocalEntryState::Present,
            "absent" => LocalEntryState::Absent,
            found => return Err(unreadable(OPERATION, "local file state", found)),
        },
    })
}

/// One row of `pending_uploads`.
pub(crate) fn pending_upload(row: &Row<'_>) -> IndexResult<PendingUpload> {
    const OPERATION: &str = "reading a spool";
    Ok(PendingUpload {
        container_id: container_id(row, "container_id", OPERATION)?,
        spool_path: PathBuf::from(text(row, "spool_path", OPERATION)?),
        batch: BatchId::new(text(row, "batch", OPERATION)?),
        created_at: DeviceTime::from_unix_seconds(integer(row, "created_at", OPERATION)?),
        state: match text(row, "state", OPERATION)?.as_str() {
            "spooling" => SpoolState::Spooling,
            "spooled" => SpoolState::Spooled,
            found => return Err(unreadable(OPERATION, "spool state", found)),
        },
        object_ref: optional_text(row, "object_ref", OPERATION)?.map(ObjectRef::new),
    })
}

fn integer(row: &Row<'_>, column: &'static str, operation: &'static str) -> IndexResult<i64> {
    row.get(column).map_err(translate(operation))
}

fn text(row: &Row<'_>, column: &'static str, operation: &'static str) -> IndexResult<String> {
    row.get(column).map_err(translate(operation))
}

fn optional_text(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<Option<String>> {
    row.get(column).map_err(translate(operation))
}

fn optional_blob(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<Option<Vec<u8>>> {
    row.get(column).map_err(translate(operation))
}

/// An Entry Path the catalog holds, which is already the NFC spelling every
/// Entry Path is in (spec: EP-1).
///
/// A row that is not is a row this build cannot read: the file is a cache that
/// can be rebuilt from Storage (spec: RV-5), so saying so and letting it be
/// discarded is the honest answer, where composing the path would quietly hand
/// callers a Library position no committed state ever held.
fn entry_path(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<EntryPath> {
    EntryPath::stored(text(row, column, operation)?).map_err(unreadable_model(operation))
}

/// The same, for a column that may hold no path at all.
fn optional_entry_path(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<Option<EntryPath>> {
    optional_text(row, column, operation)?
        .map(EntryPath::stored)
        .transpose()
        .map_err(unreadable_model(operation))
}

fn container_id(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<ContainerId> {
    let bytes: Vec<u8> = row.get(column).map_err(translate(operation))?;
    ContainerId::from_slice(&bytes).map_err(unreadable_model(operation))
}

fn content_hash(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<ContentHash> {
    let bytes: Vec<u8> = row.get(column).map_err(translate(operation))?;
    ContentHash::from_slice(&bytes).map_err(unreadable_model(operation))
}
