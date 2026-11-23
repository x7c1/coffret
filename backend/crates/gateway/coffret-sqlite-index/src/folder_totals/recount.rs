use std::collections::{BTreeMap, BTreeSet};

use coffret_usecase::device_state::LocalEntryState;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use super::{ancestors, depth, write_folder, Totals};
use crate::error::classify;
use crate::rows;

/// What the Entries standing directly in one folder come to, and how many of
/// them this device has (spec: EP-10).
pub(super) const COUNT_FILES: &str = "SELECT count(*), count(local_entries.path),
         ifnull(sum(entries.size), 0),
         ifnull(sum(CASE WHEN local_entries.path IS NULL THEN 0 ELSE entries.size END), 0),
         max(entries.mtime)
     FROM entries
     LEFT JOIN local_entries ON local_entries.path = entries.path
         AND local_entries.state = ?2 AND local_entries.relocate_to IS NULL
     WHERE entries.parent = ?1";

/// What one folder's child folders come to together.
pub(super) const COUNT_CHILD_FOLDERS: &str = "SELECT ifnull(sum(files_total), 0),
         ifnull(sum(files_present), 0), ifnull(sum(size), 0), ifnull(sum(size_present), 0),
         max(mtime)
     FROM folder_totals WHERE parent = ?1";

/// Recounts every folder above each of `paths`, deepest first, after a write
/// that may have changed the Entry or the present record at each of them.
///
/// A folder whose last Entry has gone loses its row: a folder exists exactly
/// where a current Entry stands under it (spec: EP-2).
pub(crate) fn recount<'p>(
    connection: &Connection,
    paths: impl IntoIterator<Item = &'p str>,
) -> IndexResult<()> {
    let mut by_depth: BTreeMap<usize, BTreeSet<&str>> = BTreeMap::new();
    for path in paths {
        for folder in ancestors(path) {
            if !by_depth.entry(depth(folder)).or_default().insert(folder) {
                // Every folder above it is in already, put there by whichever
                // path first stood under it.
                break;
            }
        }
    }
    // A folder's figures are read off its children's rows, so the children
    // are recounted first.
    for folders in by_depth.values().rev() {
        for folder in folders {
            recount_folder(connection, folder)?;
        }
    }
    Ok(())
}

fn recount_folder(connection: &Connection, folder: &str) -> IndexResult<()> {
    const OPERATION: &str = "recounting a folder's totals";
    let read = |row: &rusqlite::Row<'_>| -> rusqlite::Result<Totals> {
        Ok(Totals {
            files_total: row.get(0)?,
            files_present: row.get(1)?,
            size: row.get(2)?,
            size_present: row.get(3)?,
            mtime: row.get(4)?,
        })
    };
    let mut totals = connection
        .prepare_cached(COUNT_FILES)
        .and_then(|mut statement| {
            statement.query_row(
                params![folder, rows::state_text(LocalEntryState::Present)],
                read,
            )
        })
        .map_err(classify(OPERATION))?;
    totals.add(
        connection
            .prepare_cached(COUNT_CHILD_FOLDERS)
            .and_then(|mut statement| statement.query_row(params![folder], read))
            .map_err(classify(OPERATION))?,
    );
    write_folder(connection, folder, totals)
}
