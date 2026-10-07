use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{stamp, unsupported, SCHEMA_VERSION};
use crate::error::classify;

/// Adds the evidence marker without losing any device state. Rows written by
/// the older implementation may already have committed, so their marker starts
/// true. The transaction leaves either complete layout after interruption.
pub(super) fn preserve_pending_provenance(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "preserving pending provenance from layout 7";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        7 => {}
        found => return Err(unsupported(found)),
    }
    transaction.execute_batch("ALTER TABLE pending_rows ADD COLUMN commit_attempted INTEGER NOT NULL DEFAULT 1 CHECK (commit_attempted IN (0, 1));")
        .map_err(classify(OPERATION))?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify(OPERATION))?;
    transaction.commit().map_err(classify(OPERATION))
}
