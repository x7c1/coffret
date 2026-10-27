use std::collections::BTreeSet;

use coffret_model::{ContainerId, EntryNumber, EntryPath};
use coffret_usecase::catalog_renames::{check_renames, Named};
use coffret_usecase::device_state::{LocalObservation, ReservationId};
use coffret_usecase::{CommittedBatch, IndexError, IndexResult, JournalRecord};
use rusqlite::{params, Connection};

use super::{
    entry_at, insert_container, insert_entry, read_checkpoint, write_checkpoint, write_library_name,
};
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
    let library_name = record.library_name().cloned();
    let added: Vec<ContainerId> = record
        .additions()
        .iter()
        .map(|addition| addition.container().id)
        .collect();
    // Where the Entries of the rows this record's removals reach stand, read
    // before they are taken out: a replacement takes a row over at the path its
    // Entry stood at (spec: EP-6, EP-10).
    let stood = device_state::standing_in(connection, record.removals())?;

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
    // The rows a record can change the standing of: those whose Entry it
    // removes or renames, and those at a path it gives an Entry, by a rename or
    // an addition (spec: EP-10, FM-15).
    let mut reached: BTreeSet<String> = stood.iter().map(|(path, _)| path.clone()).collect();
    for one in &renamed {
        reached.extend(device_state::copies_of(
            connection,
            one.before.container_id,
            one.before.entry_number.get(),
        )?);
        reached.insert(one.path.as_str().to_owned());
    }
    for addition in record.into_additions() {
        // Each Entry keeps the number its place in the table gives it
        // (spec: FM-9, FM-15).
        let (container, located) = addition.into_locations();
        insert_container(connection, &container)?;
        for entry in &located {
            insert_entry(connection, entry)?;
            reached.insert(entry.path().as_str().to_owned());
        }
    }
    device_state::carry_over(connection, &stood, |container_id| {
        added
            .iter()
            .any(|added| added.as_bytes().as_slice() == container_id)
    })?;
    device_state::reconcile(
        connection,
        reached.iter().map(String::as_str),
        "keeping the local files' rows about their Entries",
    )?;
    // The checkpoint the record reaches; which Snapshot this catalog once
    // adopted is unchanged by a replay.
    write_checkpoint(connection, &checkpoint)?;
    // The latest record carrying a name names the Library, and one carrying
    // none leaves the name as it was (spec: FM-15).
    match &library_name {
        Some(name) => write_library_name(connection, Some(name)),
        None => Ok(()),
    }
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
    let CommittedBatch {
        record,
        materialized,
        released,
    } = batch;
    let uploaded = uploaded(&record);
    apply(connection, record)?;
    complete(connection, &uploaded, materialized, released)
}

/// Records the device-local half of this device's own batch, whose record the
/// catalog holds already (spec: EP-10, OC-2, PK-17).
///
/// The record is read for the Containers it adds and not applied; the
/// catalog and its checkpoint stay as they are.
pub(crate) fn complete_applied(connection: &Connection, batch: CommittedBatch) -> IndexResult<()> {
    complete(
        connection,
        &uploaded(&batch.record),
        batch.materialized,
        batch.released,
    )
}

/// The Containers a record adds, whose pending rows its commit clears.
fn uploaded(record: &JournalRecord) -> Vec<ContainerId> {
    record
        .additions()
        .iter()
        .map(|addition| addition.container().id)
        .collect()
}

/// The device-local half of a refresh: the files it put on disk become
/// `present` (spec: EP-10), its spools stop being pending (spec: OC-2), and the
/// reservations its batch packs are released (spec: PK-17).
fn complete(
    connection: &Connection,
    uploaded: &[ContainerId],
    materialized: Vec<LocalObservation>,
    released: Vec<ReservationId>,
) -> IndexResult<()> {
    for observation in materialized {
        device_state::mark_present(connection, &observation)?;
    }
    for &container_id in uploaded {
        device_state::clear_pending_row(connection, container_id)?;
    }
    // In the same transaction as the rows they complete, so a drop's Pack is
    // never committed here while its reservation still says it waits for one
    // (spec: PK-17).
    for id in released {
        device_state::release_reservation(connection, id)?;
    }
    Ok(())
}
