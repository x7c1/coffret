use coffret_usecase::IndexResult;
use rusqlite::Connection;

use crate::error::classify;

/// How many current Entries the catalog holds, counted by SQLite rather than
/// read.
pub(crate) fn entry_count(connection: &Connection) -> IndexResult<u64> {
    const OPERATION: &str = "counting the current Entries";
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
        .map_err(classify(OPERATION))?;
    // A count is never negative.
    Ok(count.unsigned_abs())
}
