use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{stamp, unsupported, RESERVATIONS_DDL, SCHEMA_VERSION};
use crate::error::classify;

/// Brings a layout-17 file to this one, keeping its catalog and every row of
/// its device-local group, and laying out the tables of reservations empty: no
/// build before layout 18 recorded a drop's choice of a Pack, so nothing the
/// file holds waits for one (spec: PK-17).
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
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify("stamping the Index file's schema version"))?;
    transaction.commit().map_err(classify(OPERATION))
}
