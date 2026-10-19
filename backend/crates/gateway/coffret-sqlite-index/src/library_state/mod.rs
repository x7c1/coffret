//! The three tables an Index Snapshot carries, and the operations that move
//! them from one committed Library state to the next.
//!
//! Every one of these runs inside a caller's transaction, which is what makes a
//! replay all-or-nothing: a record's removals, renames and additions take
//! effect exactly when the record commits, never partially, and a rename the
//! catalog refuses rolls the rest of the record back with it (spec: CP-1,
//! FM-15).

use coffret_model::{
    ContainerSummary, ControlObjectName, EntryLocation, IndexCheckpoint, ObjectRef,
};
use coffret_usecase::{IndexError, IndexResult};
use rusqlite::{params, Connection};

use crate::error::{classify, violation, Violation};
use crate::query::first;
use crate::rows;

mod apply;
pub(crate) use apply::{apply, refresh};

mod lookup;
pub(crate) use lookup::{containers_under, entries_in, entries_under, entry_at};

mod restore;
pub(crate) use restore::restore;

mod snapshot;
pub(crate) use snapshot::{checkpoint, snapshot};

fn read_checkpoint(
    connection: &Connection,
) -> IndexResult<Option<(IndexCheckpoint, Option<ControlObjectName>)>> {
    first(
        connection,
        "SELECT * FROM checkpoint",
        [],
        "reading the checkpoint",
        rows::checkpoint,
    )
}

/// Writes the state a restore or a replay reached, leaving the record of which
/// Snapshot was adopted as it was.
fn write_checkpoint(connection: &Connection, checkpoint: &IndexCheckpoint) -> IndexResult<()> {
    const OPERATION: &str = "writing the checkpoint";
    connection
        .execute(
            "INSERT INTO checkpoint (
                 only_row, master_key_epoch, head_generation, journal_generation,
                 next_commit_slot, keyring_generation, keyring_replica_count,
                 keyring_set_digest, adopted_snapshot
             ) VALUES (0, ?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)
             ON CONFLICT (only_row) DO UPDATE SET
                 master_key_epoch = excluded.master_key_epoch,
                 head_generation = excluded.head_generation,
                 journal_generation = excluded.journal_generation,
                 next_commit_slot = excluded.next_commit_slot,
                 keyring_generation = excluded.keyring_generation,
                 keyring_replica_count = excluded.keyring_replica_count,
                 keyring_set_digest = excluded.keyring_set_digest",
            params![
                rows::to_integer(
                    OPERATION,
                    "master_key_epoch",
                    checkpoint.master_key_epoch().get(),
                )?,
                rows::to_integer(
                    OPERATION,
                    "head_generation",
                    checkpoint.head_generation().get()
                )?,
                rows::to_integer(
                    OPERATION,
                    "journal_generation",
                    checkpoint.journal_generation().get(),
                )?,
                checkpoint.next_commit_slot(),
                rows::to_integer(
                    OPERATION,
                    "keyring_generation",
                    checkpoint.keyring().generation().get(),
                )?,
                i64::from(checkpoint.keyring().replica_count()),
                checkpoint.keyring().set_digest(),
            ],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}

fn write_adopted_from(
    connection: &Connection,
    adopted_from: Option<&ControlObjectName>,
) -> IndexResult<()> {
    connection
        .execute(
            "UPDATE checkpoint SET adopted_snapshot = ?1",
            params![adopted_from.map(ControlObjectName::to_string)],
        )
        .map_err(classify("recording which Snapshot was adopted"))?;
    Ok(())
}

fn insert_container(connection: &Connection, container: &ContainerSummary) -> IndexResult<()> {
    const OPERATION: &str = "adding a Container";
    connection
        .execute(
            "INSERT INTO containers (id, kind, ciphertext_hash, ciphertext_len, object_ref)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                container.id.as_bytes().as_slice(),
                rows::kind_text(container.kind),
                container.ciphertext_hash.as_bytes().as_slice(),
                rows::to_integer(OPERATION, "ciphertext_len", container.ciphertext_len.get())?,
                container.object_ref.as_ref().map(ObjectRef::as_str),
            ],
        )
        .map_err(|error| match violation(&error) {
            Violation::Duplicate => IndexError::DuplicateContainer {
                container_id: container.id,
            },
            _ => classify(OPERATION)(error),
        })?;
    Ok(())
}

fn insert_entry(connection: &Connection, entry: &EntryLocation) -> IndexResult<()> {
    const OPERATION: &str = "adding an Entry";
    let derived = entry.entry.derived_from.as_ref();
    connection
        .execute(
            "INSERT INTO entries (
                 path, container_id, entry_number, \"offset\", size, mtime, btime, hash,
                 mime, derived_from_container, derived_from_path, captured_path, deposited_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                entry.entry.path.as_str(),
                entry.container_id.as_bytes().as_slice(),
                entry.entry_number.get(),
                rows::to_integer(OPERATION, "offset", entry.entry.extent.offset())?,
                rows::to_integer(OPERATION, "size", entry.entry.extent.size())?,
                entry.entry.mtime.as_unix_seconds(),
                entry.entry.btime.map(|btime| btime.as_unix_seconds()),
                entry.entry.hash.as_bytes().as_slice(),
                entry.entry.mime.as_deref(),
                derived.map(|from| from.container_id.as_bytes().as_slice()),
                derived.map(|from| from.path.as_str()),
                entry.entry.captured_path.as_str(),
                entry.deposited_at.as_unix_seconds(),
            ],
        )
        .map_err(|error| match violation(&error) {
            Violation::Duplicate => IndexError::DuplicatePath {
                path: entry.entry.path.clone(),
            },
            Violation::Missing => IndexError::UnknownContainer {
                container_id: entry.container_id,
            },
            Violation::None => classify(OPERATION)(error),
        })?;
    Ok(())
}
