use coffret_model::{EntryLocation, EntryPath};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::path_prefix::subtree_range;
use crate::query::collect;
use crate::rows;

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
