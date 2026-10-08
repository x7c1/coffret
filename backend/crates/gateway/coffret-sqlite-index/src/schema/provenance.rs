use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{stamp, unsupported, SCHEMA_VERSION};
use crate::error::classify;

/// Adds the provenance markers a layout lacks without losing any device state.
///
/// Layout 7 has neither. Rows written by that implementation may already have
/// committed, so their commit-attempt marker starts true. Layout 8 has that one
/// and not the second. Every row either older layout holds names a Container
/// this device built out of its own files — nothing else was spooled before
/// layout 9 — so its materialization marker starts true (spec: OC-7). The
/// transaction leaves either complete layout after interruption.
pub(super) fn preserve_pending_provenance(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "preserving pending provenance from an older layout";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    let found = match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        found @ (7 | 8) => found,
        found => return Err(unsupported(found)),
    };
    if found == 7 {
        transaction.execute_batch("ALTER TABLE pending_rows ADD COLUMN commit_attempted INTEGER NOT NULL DEFAULT 1 CHECK (commit_attempted IN (0, 1));")
            .map_err(classify(OPERATION))?;
    }
    transaction
        .execute_batch("ALTER TABLE pending_rows ADD COLUMN materializes INTEGER NOT NULL DEFAULT 1 CHECK (materializes IN (0, 1));")
        .map_err(classify(OPERATION))?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify(OPERATION))?;
    transaction.commit().map_err(classify(OPERATION))
}
