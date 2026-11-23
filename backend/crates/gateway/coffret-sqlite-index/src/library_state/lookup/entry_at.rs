use coffret_model::{EntryLocation, EntryPath};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::query::first;
use crate::rows;

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
