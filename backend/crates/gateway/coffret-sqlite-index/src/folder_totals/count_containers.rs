use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::classify;
use crate::rows;

/// Moves the count of current Containers by what one write added and removed.
pub(crate) fn count_containers(
    connection: &Connection,
    added: usize,
    removed: usize,
) -> IndexResult<()> {
    const OPERATION: &str = "counting the current Containers";
    // A `usize` is never wider than 64 bits, so the casts lose nothing.
    connection
        .execute(
            "UPDATE catalog_counts SET containers = containers + ?1 - ?2",
            params![
                rows::to_integer(OPERATION, "containers", added as u64)?,
                rows::to_integer(OPERATION, "containers", removed as u64)?,
            ],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}
