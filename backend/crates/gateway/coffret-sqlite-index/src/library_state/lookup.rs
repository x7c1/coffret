use coffret_model::{ContainerId, ContainerSummary, EntryLocation, EntryPath};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::path_prefix::subtree_range;
use crate::query::{collect, first};
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
