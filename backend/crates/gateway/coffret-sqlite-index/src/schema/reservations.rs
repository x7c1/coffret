use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{
    replace_the_catalog, report_the_discard, stamp, unsupported, RESERVATIONS_DDL, SCHEMA_VERSION,
};
use crate::error::classify;

/// Brings a layout-17 file to this one, keeping every row of its device-local
/// group, laying out the tables of reservations empty — no build before
/// layout 18 recorded a drop's choice of a Pack, so nothing the file holds
/// waits for one (spec: PK-17) — and discarding its catalog, whose Entries
/// carry no deposit time, for the next catch-up to rebuild from Storage
/// (spec: RV-5, FM-15).
///
/// One `BEGIN IMMEDIATE` covers it, from re-reading the stamp to writing the new
/// one, for the reason a discard is one: a second process opening the file at
/// the same moment reads the stamp the first left and does nothing, and a crash
/// part-way leaves the layout-17 file as it was.
pub(super) fn upgrade(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "adding the reservations to an older layout";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        17 => {}
        found => return Err(unsupported(found)),
    }
    transaction
        .execute_batch(RESERVATIONS_DDL)
        .map_err(classify(OPERATION))?;
    replace_the_catalog(&transaction)?;
    transaction.commit().map_err(classify(OPERATION))?;
    report_the_discard(OPERATION, 17);
    Ok(())
}
