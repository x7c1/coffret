use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerSummary};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::classify;
use crate::query::first;
use crate::rows;

/// One current Container by its ID, on the primary key.
const CONTAINER_BY_ID: &str = "SELECT * FROM containers WHERE id = ?1";

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
            CONTAINER_BY_ID,
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

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::CONTAINER_BY_ID;
    use crate::query::plan;

    // LA-48: the Container a fetch reads an Entry from, and the Containers of
    // the parcels this device holds, are looked up by their IDs: one row on the
    // primary key for each, never a scan of the table.
    #[test]
    fn a_container_is_looked_up_by_its_id() {
        let mut connection = Connection::open_in_memory().expect("an in-memory database opens");
        crate::schema::prepare(&mut connection).expect("a fresh layout lays out");
        let steps = plan(&connection, CONTAINER_BY_ID);
        assert!(
            steps.iter().all(|step| step.starts_with("SEARCH ")),
            "{CONTAINER_BY_ID} reads by {steps:?}"
        );
    }
}
