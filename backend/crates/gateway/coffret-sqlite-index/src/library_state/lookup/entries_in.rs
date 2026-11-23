use coffret_model::{ContainerId, EntryLocation};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::query::collect;
use crate::rows;

/// Every current Entry one Container holds, ordered by entry number
/// (spec: FM-9).
pub(crate) fn entries_in(
    connection: &Connection,
    container_id: ContainerId,
) -> IndexResult<Vec<EntryLocation>> {
    collect(
        connection,
        "SELECT * FROM entries WHERE container_id = ?1 ORDER BY entry_number",
        params![container_id.as_bytes().as_slice()],
        "reading a Container's Entries",
        rows::entry_location,
    )
}
