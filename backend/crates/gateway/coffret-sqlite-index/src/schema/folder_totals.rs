use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{stamp, unsupported, ENTRIES_DDL, FOLDER_TOTALS_DDL, SCHEMA_VERSION};
use crate::error::classify;
use crate::folder_totals::rebuild;

/// Brings a layout-21 file to this one, keeping every row it holds: each Entry
/// gains the folder it stands directly in, and every folder's totals and the
/// count of current Containers are laid out from the rows the file holds
/// (spec: LA-45).
///
/// Built from those rows rather than discarded with the catalog: the totals
/// are a derivation of the catalog and this device's present materialization
/// records, both of which the file already holds, and discarding the catalog
/// would cost this device the records a replay from the Library's first head
/// cannot tie back to their Entries (spec: EP-10).
///
/// One `BEGIN IMMEDIATE` covers it, from re-reading the stamp to writing the
/// new one, for the reason a discard is one: a second process opening the file
/// at the same moment reads the stamp the first left and does nothing, and a
/// crash part-way leaves the layout-21 file as it was.
pub(super) fn upgrade(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "adding the folders' totals to an older layout";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        21 => {}
        found => return Err(unsupported(found)),
    }
    add_the_totals(&transaction)?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify("stamping the Index file's schema version"))?;
    transaction.commit().map_err(classify(OPERATION))
}

/// Gives each Entry the folder it stands directly in and lays out every
/// folder's totals from the rows the file holds, inside the caller's
/// transaction.
///
/// The table of Entries is laid out afresh and its rows copied into it, so the
/// file ends with the very table a fresh one is created with — the column in
/// its place and no default on it — rather than one a column was added to.
pub(super) fn add_the_totals(transaction: &Connection) -> IndexResult<()> {
    const OPERATION: &str = "giving each Entry the folder it stands in";
    // The index goes with the old table's name otherwise, and the new table
    // lays out one of the same name.
    transaction
        .execute_batch(
            "DROP INDEX entries_by_container;
             ALTER TABLE entries RENAME TO entries_before_parent;",
        )
        .map_err(classify(OPERATION))?;
    transaction
        .execute_batch(ENTRIES_DDL)
        .map_err(classify(OPERATION))?;
    // Each Entry's folder is cut out of its path by the expression the table's
    // check holds `parent` to, so every copied row meets it.
    transaction
        .execute_batch(
            "INSERT INTO entries (
                 path, parent, captured_path, container_id, entry_number, \"offset\", size,
                 mtime, btime, deposited_at, hash, mime, derived_from_container,
                 derived_from_path
             )
             SELECT path, rtrim(rtrim(path, replace(path, '/', '')), '/'), captured_path,
                 container_id, entry_number, \"offset\", size, mtime, btime, deposited_at,
                 hash, mime, derived_from_container, derived_from_path
             FROM entries_before_parent;
             DROP TABLE entries_before_parent;",
        )
        .map_err(classify(OPERATION))?;
    transaction
        .execute_batch(FOLDER_TOTALS_DDL)
        .map_err(classify("laying out the folders' totals"))?;
    rebuild(transaction)
}
