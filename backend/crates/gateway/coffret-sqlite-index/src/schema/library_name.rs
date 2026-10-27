use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{stamp, unsupported, SCHEMA_VERSION};
use crate::error::classify;

/// Gives a layout-20 file's checkpoint row the Library's name, unset, keeping
/// every row it holds (spec: FM-15, FM-16).
///
/// Unset rather than read from anywhere: a build before layout 21 kept no
/// name, and the catalog is kept rather than rebuilt — rebuilding it would
/// cost this device the materialization records a replay from the Library's
/// first head cannot tie back to their Entries (spec: EP-10). A name the
/// Library already has is read from Storage by the next open, before it gives
/// the Library one of its own.
///
/// One `BEGIN IMMEDIATE` covers it, from re-reading the stamp to writing the
/// new one, for the reason a discard is one: a second process opening the file
/// at the same moment reads the stamp the first left and does nothing, and a
/// crash part-way leaves the layout-20 file as it was.
pub(super) fn add(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "adding the Library's name to an older layout";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        20 => {}
        found => return Err(unsupported(found)),
    }
    add_the_column(&transaction)?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify("stamping the Index file's schema version"))?;
    transaction.commit().map_err(classify(OPERATION))
}

/// Adds the column, inside the caller's transaction.
pub(super) fn add_the_column(transaction: &Connection) -> IndexResult<()> {
    transaction
        .execute_batch("ALTER TABLE checkpoint ADD COLUMN library_name TEXT;")
        .map_err(classify("adding the Library's name to the checkpoint row"))
}
