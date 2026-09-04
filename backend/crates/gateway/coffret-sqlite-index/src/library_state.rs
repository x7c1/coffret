//! The three tables an Index Snapshot carries, and the operations that move
//! them from one committed Library state to the next.
//!
//! Every one of these runs inside a caller's transaction, which is what makes a
//! replay all-or-nothing: a record's additions and removals take effect exactly
//! when the record commits, never partially (spec: CP-1).

use coffret_model::{
    ContainerId, ContainerSummary, ControlObjectName, EntryLocation, EntryPath, IndexCheckpoint,
    ObjectRef,
};
use coffret_usecase::{CommittedBatch, IndexError, IndexResult, JournalRecord, SnapshotContent};
use rusqlite::{params, Connection};

use crate::device_state;
use crate::error::{translate, violation, Violation};
use crate::path_prefix::subtree_range;
use crate::query::{collect, first};
use crate::rows;

/// Replaces the whole Library-wide state with a Snapshot's content
/// (spec: CK-7, CK-9, RV-1).
pub(crate) fn restore(connection: &Connection, snapshot: SnapshotContent) -> IndexResult<()> {
    // Entries first: each one refers to a Container.
    connection
        .execute("DELETE FROM entries", [])
        .map_err(translate("clearing the Entries"))?;
    connection
        .execute("DELETE FROM containers", [])
        .map_err(translate("clearing the Containers"))?;

    for container in snapshot.containers {
        insert_container(connection, &container)?;
    }
    for entry in snapshot.entries {
        insert_entry(connection, &entry)?;
    }
    write_checkpoint(connection, &snapshot.checkpoint)?;
    write_adopted_from(connection, snapshot.adopted_from.as_ref())
}

/// Replays one committed Journal record (spec: CP-1, CP-11, EP-6).
pub(crate) fn apply(connection: &Connection, record: JournalRecord) -> IndexResult<()> {
    let checkpoint = record.checkpoint();

    // Removals leave the current set before additions enter it: within one
    // record a path may move from a replaced Container to its replacement
    // (spec: EP-6).
    for removed in &record.removals {
        connection
            .execute(
                "DELETE FROM entries WHERE container_id = ?1",
                params![removed.as_bytes().as_slice()],
            )
            .map_err(translate("removing a Container's Entries"))?;
        connection
            .execute(
                "DELETE FROM containers WHERE id = ?1",
                params![removed.as_bytes().as_slice()],
            )
            .map_err(translate("removing a Container"))?;
    }
    for addition in record.additions {
        let container_id = addition.container.id;
        insert_container(connection, &addition.container)?;
        for entry in addition.entries {
            insert_entry(
                connection,
                &EntryLocation {
                    container_id,
                    entry,
                },
            )?;
        }
    }
    // The checkpoint the record reaches; which Snapshot this catalog once
    // adopted is unchanged by a replay.
    write_checkpoint(connection, &checkpoint)
}

/// Applies this device's own committed batch (spec: CP-1, EP-10, OC-2).
pub(crate) fn refresh(connection: &Connection, batch: CommittedBatch) -> IndexResult<()> {
    let uploaded: Vec<ContainerId> = batch
        .record
        .additions
        .iter()
        .map(|addition| addition.container.id)
        .collect();

    apply(connection, batch.record)?;

    for observation in batch.materialized {
        device_state::mark_present(connection, &observation)?;
    }
    for container_id in uploaded {
        device_state::clear_pending_upload(connection, container_id)?;
    }
    Ok(())
}

/// The whole Library-wide state, in canonical order (spec: CK-8, EP-3).
pub(crate) fn snapshot(connection: &Connection) -> IndexResult<SnapshotContent> {
    let (checkpoint, adopted_from) =
        read_checkpoint(connection)?.ok_or(IndexError::NoCheckpoint)?;
    Ok(SnapshotContent {
        checkpoint,
        adopted_from,
        containers: collect(
            connection,
            "SELECT * FROM containers ORDER BY id",
            [],
            "reading the Containers",
            rows::container_summary,
        )?,
        entries: collect(
            connection,
            "SELECT * FROM entries ORDER BY path",
            [],
            "reading the Entries",
            rows::entry_location,
        )?,
    })
}

/// The committed Library state this catalog stands at (spec: CK-9).
pub(crate) fn checkpoint(connection: &Connection) -> IndexResult<Option<IndexCheckpoint>> {
    Ok(read_checkpoint(connection)?.map(|(checkpoint, _)| checkpoint))
}

/// The current Entry at one Entry Path, of which there is at most one
/// (spec: EP-5).
pub(crate) fn entry_at(
    connection: &Connection,
    path: &EntryPath,
) -> IndexResult<Option<EntryLocation>> {
    first(
        connection,
        "SELECT * FROM entries WHERE path = ?1",
        params![path.as_str()],
        "reading an Entry",
        rows::entry_location,
    )
}

/// Every current Entry under a prefix, ordered by Entry Path bytes.
pub(crate) fn entries_under(
    connection: &Connection,
    prefix: Option<&EntryPath>,
) -> IndexResult<Vec<EntryLocation>> {
    const OPERATION: &str = "reading a subtree's Entries";
    match prefix {
        None => collect(
            connection,
            "SELECT * FROM entries ORDER BY path",
            [],
            OPERATION,
            rows::entry_location,
        ),
        Some(prefix) => {
            let (lower, upper) = subtree_range(prefix);
            collect(
                connection,
                "SELECT * FROM entries \
                 WHERE path = ?1 OR (path >= ?2 AND path < ?3) \
                 ORDER BY path",
                params![prefix.as_str(), lower, upper],
                OPERATION,
                rows::entry_location,
            )
        }
    }
}

/// The distinct Containers holding any Entry under a prefix (spec: PK-8).
pub(crate) fn containers_under(
    connection: &Connection,
    prefix: Option<&EntryPath>,
) -> IndexResult<Vec<ContainerSummary>> {
    const OPERATION: &str = "reading a subtree's Containers";
    match prefix {
        None => collect(
            connection,
            "SELECT DISTINCT containers.* FROM containers \
             JOIN entries ON entries.container_id = containers.id \
             ORDER BY containers.id",
            [],
            OPERATION,
            rows::container_summary,
        ),
        Some(prefix) => {
            let (lower, upper) = subtree_range(prefix);
            collect(
                connection,
                "SELECT DISTINCT containers.* FROM containers \
                 JOIN entries ON entries.container_id = containers.id \
                 WHERE entries.path = ?1 OR (entries.path >= ?2 AND entries.path < ?3) \
                 ORDER BY containers.id",
                params![prefix.as_str(), lower, upper],
                OPERATION,
                rows::container_summary,
            )
        }
    }
}

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
                rows::to_integer(checkpoint.master_key_epoch.get()),
                rows::to_integer(checkpoint.head_generation.get()),
                rows::to_integer(checkpoint.journal_generation.get()),
                checkpoint.next_commit_slot.as_deref(),
                rows::to_integer(checkpoint.keyring.generation().get()),
                i64::from(checkpoint.keyring.replica_count()),
                checkpoint.keyring.set_digest(),
            ],
        )
        .map_err(translate("writing the checkpoint"))?;
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
        .map_err(translate("recording which Snapshot was adopted"))?;
    Ok(())
}

fn insert_container(connection: &Connection, container: &ContainerSummary) -> IndexResult<()> {
    connection
        .execute(
            "INSERT INTO containers (id, kind, ciphertext_hash, ciphertext_len, object_ref)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                container.id.as_bytes().as_slice(),
                rows::kind_text(container.kind),
                container.ciphertext_hash.as_bytes().as_slice(),
                rows::to_integer(container.ciphertext_len.get()),
                container.object_ref.as_ref().map(ObjectRef::as_str),
            ],
        )
        .map_err(|error| match violation(&error) {
            Violation::Duplicate => IndexError::DuplicateContainer {
                container_id: container.id,
            },
            _ => translate("adding a Container")(error),
        })?;
    Ok(())
}

fn insert_entry(connection: &Connection, entry: &EntryLocation) -> IndexResult<()> {
    let derived = entry.entry.derived_from.as_ref();
    connection
        .execute(
            "INSERT INTO entries (
                 path, container_id, \"offset\", size, mtime, btime, hash, mime,
                 derived_from_container, derived_from_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.entry.path.as_str(),
                entry.container_id.as_bytes().as_slice(),
                rows::to_integer(entry.entry.extent.offset()),
                rows::to_integer(entry.entry.extent.size()),
                entry.entry.mtime.as_unix_seconds(),
                entry.entry.btime.map(|btime| btime.as_unix_seconds()),
                entry.entry.hash.as_bytes().as_slice(),
                entry.entry.mime.as_deref(),
                derived.map(|from| from.container_id.as_bytes().as_slice()),
                derived.map(|from| from.path.as_str()),
            ],
        )
        .map_err(|error| match violation(&error) {
            Violation::Duplicate => IndexError::DuplicatePath {
                path: entry.entry.path.clone(),
            },
            Violation::Missing => IndexError::UnknownContainer {
                container_id: entry.container_id,
            },
            Violation::None => translate("adding an Entry")(error),
        })?;
    Ok(())
}
