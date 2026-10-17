use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{
    replace_the_catalog, report_the_discard, stamp, unsupported, ATTEMPTED_SLOTS_DDL,
    HELD_PARCELS_DDL, SCHEMA_VERSION,
};
use crate::error::classify;

/// Adds the columns an older layout lacks without losing any device state, or
/// any catalog row.
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
/// (spec: PK-21). Layouts 7 to 11 recorded no commit slot, so the table of
/// attempted slots starts empty, and a row they marked attempted stays one
/// whose outcome is unknown: nothing says where its create may have landed
/// (spec: OC-2, OC-3). The catalog every one of these layouts holds is not
/// carried forward: it was filled from Storage objects in a format this build no
/// longer reads, so it is discarded and the next catch-up rebuilds it from
/// Storage (spec: RV-5). The transaction leaves either complete layout after
/// interruption.
pub(super) fn preserve_device_state(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "preserving device state from an older layout";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    let found = match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        found @ 7..=12 => found,
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
    if found <= 10 {
        transaction
            .execute_batch(HELD_PARCELS_DDL)
            .map_err(classify(OPERATION))?;
    }
    if found <= 11 {
        transaction
            .execute_batch(ATTEMPTED_SLOTS_DDL)
            .map_err(classify(OPERATION))?;
    }
    replace_the_catalog(&transaction)?;
    transaction.commit().map_err(classify(OPERATION))?;
    report_the_discard(OPERATION, found);
    Ok(())
}
