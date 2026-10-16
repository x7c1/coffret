use coffret_usecase::IndexResult;
use rusqlite::Connection;

use super::ENTRIES_DDL;
use crate::error::classify;

/// Lays the table of current Entries out again with the original path a layout
/// before 13 lacked, each row's taken to be its current one (spec: FM-15,
/// FM-16).
///
/// SQLite adds a `NOT NULL` column only with a default, and a default would
/// leave this file's table spelled differently from a fresh one's. So the old
/// table is moved aside, the current one laid out under its name, and the rows
/// copied across. Nothing refers to `entries`, so moving it aside breaks no
/// reference; its index goes with it and is laid out again with the table.
pub(super) fn add_original_paths(transaction: &Connection) -> IndexResult<()> {
    const OPERATION: &str = "adding the original path to every cataloged Entry";
    transaction
        .execute_batch(
            "ALTER TABLE entries RENAME TO entries_before_original_path; \
             DROP INDEX entries_by_container;",
        )
        .map_err(classify(OPERATION))?;
    transaction
        .execute_batch(ENTRIES_DDL)
        .map_err(classify(OPERATION))?;
    transaction
        .execute_batch(
            "INSERT INTO entries ( \
                 path, original_path, container_id, \"offset\", size, mtime, btime, hash, \
                 mime, derived_from_container, derived_from_path \
             ) \
             SELECT path, path, container_id, \"offset\", size, mtime, btime, hash, \
                 mime, derived_from_container, derived_from_path \
             FROM entries_before_original_path; \
             DROP TABLE entries_before_original_path;",
        )
        .map_err(classify(OPERATION))
}
