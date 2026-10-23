use std::collections::BTreeMap;

use coffret_model::EntryPath;
use coffret_usecase::device_state::{Reservation, ReservationId};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::classify;
use crate::query::collect;
use crate::rows;

/// Records a drop's choice to add its files to `folder` as a Pack, naming no
/// path yet, and answers with the identifier the row was drawn (spec: PK-17).
///
/// `AUTOINCREMENT` is what keeps an identifier from being drawn twice, a
/// released one's included, so the order of the identifiers is the order the
/// reservations were made in.
pub(crate) fn reserve(
    connection: &Connection,
    folder: Option<&EntryPath>,
) -> IndexResult<ReservationId> {
    const OPERATION: &str = "recording a reservation";
    connection
        .execute(
            "INSERT INTO reservations (folder) VALUES (?1)",
            params![folder.map(EntryPath::as_str)],
        )
        .map_err(classify(OPERATION))?;
    let id = connection.last_insert_rowid();
    // A row id SQLite drew is positive, so this reads it back the way every
    // stored count is read, refusing what could only be a damaged file.
    u64::try_from(id)
        .map(ReservationId::new)
        .map_err(|_| crate::error::negative(OPERATION, "id", id))
}

/// Adds a path to a reservation. One that is gone changes nothing, and none is
/// made up for it (spec: PK-17).
pub(crate) fn extend_reservation(
    connection: &Connection,
    id: ReservationId,
    path: &EntryPath,
) -> IndexResult<()> {
    const OPERATION: &str = "extending a reservation";
    connection
        .execute(
            "INSERT OR IGNORE INTO reserved_paths (reservation, path)
             SELECT id, ?2 FROM reservations WHERE id = ?1",
            params![
                rows::to_integer(OPERATION, "reservation", id.get())?,
                path.as_str()
            ],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}

/// Marks a reservation received, its drop having stopped receiving. One that is
/// gone changes nothing, and none is made up for it (spec: PK-17).
pub(crate) fn mark_received(connection: &Connection, id: ReservationId) -> IndexResult<()> {
    const OPERATION: &str = "marking a reservation received";
    connection
        .execute(
            "UPDATE reservations SET received = 1 WHERE id = ?1",
            params![rows::to_integer(OPERATION, "id", id.get())?],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}

/// Every reservation, in the order they were made, each with every path it
/// names and whether its drop finished receiving (spec: PK-17).
pub(crate) fn reservations(connection: &Connection) -> IndexResult<Vec<Reservation>> {
    let mut reservations: BTreeMap<ReservationId, Reservation> = collect(
        connection,
        "SELECT id, folder, received FROM reservations ORDER BY id",
        [],
        "reading the reservations",
        rows::reservation,
    )?
    .into_iter()
    .map(|(id, folder, received)| {
        (
            id,
            Reservation {
                id,
                folder,
                paths: Default::default(),
                received,
            },
        )
    })
    .collect();
    for (id, path) in collect(
        connection,
        "SELECT reservation, path FROM reserved_paths ORDER BY reservation, path",
        [],
        "reading the reserved paths",
        rows::reserved_path,
    )? {
        if let Some(reservation) = reservations.get_mut(&id) {
            reservation.paths.insert(path);
        }
    }
    Ok(reservations.into_values().collect())
}

/// Releases one reservation, removing it and the paths it names; one that is
/// not there is a no-op, so an interrupted clean-up is simply run again
/// (spec: OC-8, PK-17).
pub(crate) fn release_reservation(connection: &Connection, id: ReservationId) -> IndexResult<()> {
    const OPERATION: &str = "releasing a reservation";
    let id = rows::to_integer(OPERATION, "id", id.get())?;
    // The paths before the row they name: foreign keys are on for this
    // connection, so the row cannot go first.
    connection
        .execute(
            "DELETE FROM reserved_paths WHERE reservation = ?1",
            params![id],
        )
        .map_err(classify(OPERATION))?;
    connection
        .execute("DELETE FROM reservations WHERE id = ?1", params![id])
        .map_err(classify(OPERATION))?;
    Ok(())
}
