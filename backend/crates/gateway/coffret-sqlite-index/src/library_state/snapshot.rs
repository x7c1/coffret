use coffret_model::{IndexCheckpoint, LibraryName};
use coffret_usecase::{IndexError, IndexResult, SnapshotContent};
use rusqlite::Connection;

use super::{read_checkpoint, read_library_name};
use crate::error::unreadable_model;
use crate::query::collect;
use crate::rows;

/// The whole Library-wide state, in canonical order (spec: CK-8, EP-3).
///
/// The two `ORDER BY` clauses are how the rows arrive sorted, and
/// [`SnapshotContent::new`] is what says they are: a catalog whose rows do not
/// hold together — a Container listed twice, an Entry in a Container the file
/// no longer has — is one this build cannot read, which is the answer RV-5 asks
/// for rather than a Snapshot no reader could take.
pub(crate) fn snapshot(connection: &Connection) -> IndexResult<SnapshotContent> {
    const OPERATION: &str = "reading the Library-wide state";

    let (checkpoint, adopted_from) =
        read_checkpoint(connection)?.ok_or(IndexError::NoCheckpoint)?;
    let containers = collect(
        connection,
        "SELECT * FROM containers ORDER BY id",
        [],
        "reading the Containers",
        rows::container_summary,
    )?;
    let entries = collect(
        connection,
        "SELECT * FROM entries ORDER BY path",
        [],
        "reading the Entries",
        rows::entry_location,
    )?;

    let name = read_library_name(connection)?;

    SnapshotContent::new(checkpoint, adopted_from, containers, entries)
        .map(|content| content.with_library_name(name))
        .map_err(unreadable_model(OPERATION))
}

/// The committed Library state this catalog stands at (spec: CK-9).
pub(crate) fn checkpoint(connection: &Connection) -> IndexResult<Option<IndexCheckpoint>> {
    Ok(read_checkpoint(connection)?.map(|(checkpoint, _)| checkpoint))
}

/// The Library's name as this catalog holds it (spec: FM-15, FM-16).
pub(crate) fn library_name(connection: &Connection) -> IndexResult<Option<LibraryName>> {
    read_library_name(connection)
}
