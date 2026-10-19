use coffret_usecase::IndexResult;
use rusqlite::{Connection, TransactionBehavior};

use super::{stamp, unsupported, LOCAL_ENTRIES_DDL, RESERVATIONS_DDL, SCHEMA_VERSION};
use crate::error::classify;

/// Brings a layout-16 file to this one, keeping its catalog and every row of
/// its device-local group the catalog can name the Entry of (spec: EP-10), and
/// laying out the tables of reservations empty (spec: PK-17).
///
/// One `BEGIN IMMEDIATE` covers it, from re-reading the stamp to writing the new
/// one, for the reason a discard is one: a second process opening the file at
/// the same moment reads the stamp the first left and does nothing, and a crash
/// part-way leaves the layout-16 file as it was.
pub(super) fn upgrade(connection: &mut Connection) -> IndexResult<()> {
    const OPERATION: &str = "tying materialization records to their Entries";
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify(OPERATION))?;
    match stamp(&transaction)? {
        SCHEMA_VERSION => return Ok(()),
        16 => {}
        found => return Err(unsupported(found)),
    }
    let dropped = tie_records_to_entries(&transaction)?;
    transaction
        .execute_batch(RESERVATIONS_DDL)
        .map_err(classify(OPERATION))?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify("stamping the Index file's schema version"))?;
    transaction.commit().map_err(classify(OPERATION))?;
    report_the_drop(dropped);
    Ok(())
}

/// Lays `local_entries` out afresh with the Entry each record is a copy of,
/// filled from the catalog in the caller's transaction, and answers with how
/// many records it could not name an Entry for and dropped.
///
/// A record names the Entry the catalog holds where it waits to follow
/// (`relocate_to`) or, waiting for nothing, where it stands: the rule every
/// layout before this one read a record by, so the Entry is the one the record
/// was already taken to be about (spec: EP-10). A record the catalog holds no
/// Entry for there — a file whose Entry has left the Library, or any record at
/// all beside a catalog that numbers no Entry, which every layout before 15 is
/// (spec: FM-16) — is dropped rather than guessed at: a guess is how a record
/// comes to claim an Entry its file is not a copy of. Dropped, its file is one
/// this device never placed, which a fetch refuses to overwrite and a scan
/// reads as any other file (spec: EP-11).
///
/// The older group must already have `relocate_to` and `hash`; whatever else it
/// holds is carried over as it was.
pub(super) fn tie_records_to_entries(transaction: &Connection) -> IndexResult<u64> {
    const OPERATION: &str = "tying materialization records to their Entries";
    let numbered: bool = transaction
        .query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM pragma_table_info('entries') WHERE name = 'entry_number'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(classify(OPERATION))?;
    transaction
        .execute_batch("ALTER TABLE local_entries RENAME TO local_entries_unnamed;")
        .map_err(classify(OPERATION))?;
    transaction
        .execute_batch(LOCAL_ENTRIES_DDL)
        .map_err(classify(OPERATION))?;
    if numbered {
        transaction
            .execute_batch(
                "INSERT INTO local_entries (
                     path, state, observed_size, observed_mtime, observed_at, hash, relocate_to,
                     entry_container, entry_number
                 )
                 SELECT unnamed.path, unnamed.state, unnamed.observed_size,
                        unnamed.observed_mtime, unnamed.observed_at, unnamed.hash,
                        unnamed.relocate_to, entries.container_id, entries.entry_number
                 FROM local_entries_unnamed AS unnamed
                 JOIN entries ON entries.path = COALESCE(unnamed.relocate_to, unnamed.path);",
            )
            .map_err(classify(OPERATION))?;
    }
    let count = |table: &str| -> IndexResult<u64> {
        transaction
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get::<_, u64>(0)
            })
            .map_err(classify(OPERATION))
    };
    let dropped = count("local_entries_unnamed")? - count("local_entries")?;
    transaction
        .execute_batch("DROP TABLE local_entries_unnamed;")
        .map_err(classify(OPERATION))?;
    Ok(dropped)
}

/// Says how many materialization records an upgrade could not name an Entry
/// for and dropped, once the transaction that dropped them has committed.
///
/// A count and nothing else: a record's path is an Entry Path, which no
/// diagnostic event carries (spec: EL-1).
pub(super) fn report_the_drop(dropped: u64) {
    if dropped == 0 {
        return;
    }
    tracing::warn!(
        dropped,
        "the Index file held materialization records the catalog names no Entry for; they \
         were dropped, and the files they recorded are now ones this device never placed"
    );
}
