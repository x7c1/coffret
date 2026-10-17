use coffret_model::{ContainerId, EntryNumber, EntryPath};
use coffret_usecase::catalog_renames::{check_renames, Named};
use coffret_usecase::{CommittedBatch, IndexError, IndexResult, JournalRecord};
use rusqlite::{params, Connection};

use super::{entry_at, insert_container, insert_entry, read_checkpoint, write_checkpoint};
use crate::device_state;
use crate::error::classify;
use crate::query::first;
use crate::rows;

/// Replays one committed Journal record (spec: CP-1, CP-11, EP-6, FM-15).
pub(crate) fn apply(connection: &Connection, record: JournalRecord) -> IndexResult<()> {
    // A record at or below the head this catalog stands at is one it holds
    // already, refused before any of it is applied (spec: CK-9, RV-5). The
    // transaction holds the write lock from its start, so no other process
    // commits between this reading and the writes below (spec: CK-13).
    if let Some((at, _)) = read_checkpoint(connection)? {
        if record.generation() <= at.head_generation() {
            return Err(IndexError::AlreadyApplied {
                generation: record.generation(),
                checkpoint: at.head_generation(),
            });
        }
    }
    let checkpoint = record.checkpoint();
    let added: Vec<ContainerId> = record
        .additions()
        .iter()
        .map(|addition| addition.container().id)
        .collect();
    let vacated = device_state::following_any_of(connection, record.removals())?;

    // Removals leave the current set before additions enter it: within one
    // record a path may pass from a replaced Container to its replacement
    // (spec: EP-6).
    for removed in record.removals() {
        connection
            .execute(
                "DELETE FROM entries WHERE container_id = ?1",
                params![removed.as_bytes().as_slice()],
            )
            .map_err(classify("removing a Container's Entries"))?;
        connection
            .execute(
                "DELETE FROM containers WHERE id = ?1",
                params![removed.as_bytes().as_slice()],
            )
            .map_err(classify("removing a Container"))?;
    }
    // Then every rename at once, against what the removals left
    // (spec: FM-15). Each renamed row leaves before any comes back under its
    // new path, so two Entries may swap their names without either meeting
    // the other's row on the way.
    let renamed = check_renames(
        &record,
        |container_id, entry_number| named(connection, container_id, entry_number),
        |path| standing_at(connection, path),
    )?;
    for one in &renamed {
        connection
            .execute(
                "DELETE FROM entries WHERE container_id = ?1 AND entry_number = ?2",
                params![
                    one.before.container_id.as_bytes().as_slice(),
                    one.before.entry_number.get(),
                ],
            )
            .map_err(classify("taking a renamed Entry from its path"))?;
    }
    for one in &renamed {
        insert_entry(connection, &one.after())?;
    }
    device_state::follow_renames(connection, &renamed)?;
    for addition in record.into_additions() {
        // Each Entry keeps the number its place in the table gives it
        // (spec: FM-9, FM-15).
        let (container, located) = addition.into_locations();
        insert_container(connection, &container)?;
        for entry in &located {
            insert_entry(connection, entry)?;
        }
    }
    device_state::release_vacated(connection, &vacated, &added)?;
    // The checkpoint the record reaches; which Snapshot this catalog once
    // adopted is unchanged by a replay.
    write_checkpoint(connection, &checkpoint)
}

/// What the catalog holds under one `(Container ID, entry number)`, for a
/// rename naming it (spec: FM-9, FM-15).
fn named(
    connection: &Connection,
    container_id: ContainerId,
    entry_number: EntryNumber,
) -> IndexResult<Named> {
    let entry = first(
        connection,
        "SELECT * FROM entries WHERE container_id = ?1 AND entry_number = ?2",
        params![container_id.as_bytes().as_slice(), entry_number.get()],
        "reading the Entry a rename names",
        rows::entry_location,
    )?;
    if let Some(entry) = entry {
        return Ok(Named::Entry(Box::new(entry)));
    }
    let current = connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM containers WHERE id = ?1)",
            params![container_id.as_bytes().as_slice()],
            |row| row.get::<_, bool>(0),
        )
        .map_err(classify(
            "reading whether a renamed Entry's Container is current",
        ))?;
    Ok(if current {
        Named::NoEntry
    } else {
        Named::NoContainer
    })
}

/// Which Entry stands at one Entry Path, by the name a rename gives it.
fn standing_at(
    connection: &Connection,
    path: &EntryPath,
) -> IndexResult<Option<(ContainerId, EntryNumber)>> {
    Ok(entry_at(connection, path)?.map(|entry| (entry.container_id, entry.entry_number)))
}

/// Applies this device's own committed batch (spec: CP-1, EP-10, OC-2).
pub(crate) fn refresh(connection: &Connection, batch: CommittedBatch) -> IndexResult<()> {
    let uploaded: Vec<ContainerId> = batch
        .record
        .additions()
        .iter()
        .map(|addition| addition.container().id)
        .collect();

    apply(connection, batch.record)?;

    for observation in batch.materialized {
        device_state::mark_present(connection, &observation)?;
    }
    for container_id in uploaded {
        device_state::clear_pending_row(connection, container_id)?;
    }
    Ok(())
}
