use coffret_model::{ContainerSummary, EntryPath};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::path_prefix::subtree_range;
use crate::query::collect;
use crate::rows;

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
