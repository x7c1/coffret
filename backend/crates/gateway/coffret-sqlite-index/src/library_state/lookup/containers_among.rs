use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerSummary};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::classify;
use crate::query::first;
use crate::rows;

/// The current Containers among `ids`, ordered by Container ID: one lookup by
/// the primary key for each, rather than a read of the Library.
///
/// Every lookup runs in one read transaction, so the answer is of one moment
/// of the catalog even where another process commits between two of them
/// (spec: CK-13). The `containers` table holds the current Containers and
/// nothing else, each with at least one Entry (spec: FM-10).
pub(crate) fn containers_among(
    connection: &Connection,
    ids: &BTreeSet<ContainerId>,
) -> IndexResult<Vec<ContainerSummary>> {
    const OPERATION: &str = "reading the named Containers";
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let transaction = connection
        .unchecked_transaction()
        .map_err(classify(OPERATION))?;
    let mut found = Vec::new();
    // In the set's own order, which is Container ID order.
    for id in ids {
        if let Some(container) = first(
            &transaction,
            "SELECT * FROM containers WHERE id = ?1",
            params![id.as_bytes().as_slice()],
            OPERATION,
            rows::container_summary,
        )? {
            found.push(container);
        }
    }
    transaction.commit().map_err(classify(OPERATION))?;
    Ok(found)
}
