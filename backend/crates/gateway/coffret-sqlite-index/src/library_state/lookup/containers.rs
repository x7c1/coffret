use coffret_model::ContainerSummary;
use coffret_usecase::IndexResult;
use rusqlite::Connection;

use crate::query::collect;
use crate::rows;

/// Every current Container, ordered by Container ID, read off the `containers`
/// table alone: no Entry is read to answer it (spec: FM-10).
pub(crate) fn containers(connection: &Connection) -> IndexResult<Vec<ContainerSummary>> {
    collect(
        connection,
        "SELECT * FROM containers ORDER BY id",
        [],
        "reading the current Containers",
        rows::container_summary,
    )
}
