use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{stamp, unsupported, HELD_PARCELS_DDL, SCHEMA_VERSION};
use crate::error::classify;

/// Adds the columns an older device-local layout lacks without losing any
/// device state.
///
/// Layout 7 has neither provenance marker. Rows written by that implementation
/// may already have committed, so their commit-attempt marker starts true.
/// Layout 8 has that one and not the second. Every row either older layout
/// holds names a Container this device built out of its own files — nothing
/// else was spooled before layout 9 — so its materialization marker starts true
/// (spec: OC-7). Layouts 7 to 9 recorded no content hash beside a
/// materialization, so every row they hold keeps none, and a file whose Entry
/// later leaves the Library is then judged by its length and modification time
/// alone (spec: EP-15). Layouts 7 to 10 kept no parcel, so the table of held
/// parcels starts empty, which is a device holding nothing it read
/// (spec: PK-21). The transaction leaves either complete layout after
/// interruption.
pub(super) fn preserve_device_state(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "preserving device state from an older layout";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    let found = match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        found @ 7..=10 => found,
        found => return Err(unsupported(found)),
    };
    if found == 7 {
        transaction.execute_batch("ALTER TABLE pending_rows ADD COLUMN commit_attempted INTEGER NOT NULL DEFAULT 1 CHECK (commit_attempted IN (0, 1));")
            .map_err(classify(OPERATION))?;
    }
    if found <= 8 {
        transaction
            .execute_batch("ALTER TABLE pending_rows ADD COLUMN materializes INTEGER NOT NULL DEFAULT 1 CHECK (materializes IN (0, 1));")
            .map_err(classify(OPERATION))?;
    }
    if found <= 9 {
        transaction
            .execute_batch("ALTER TABLE local_entries ADD COLUMN hash BLOB;")
            .map_err(classify(OPERATION))?;
    }
    transaction
        .execute_batch(HELD_PARCELS_DDL)
        .map_err(classify(OPERATION))?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify(OPERATION))?;
    transaction.commit().map_err(classify(OPERATION))
}
