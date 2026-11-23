use std::collections::BTreeMap;

use coffret_usecase::device_state::LocalEntryState;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use super::{ancestors, write_folder, Totals};
use crate::error::classify;
use crate::rows;

/// Lays every folder's totals and the count of Containers out afresh from the
/// rows the file holds, after a write that replaced the whole catalog — a
/// restore — or an upgrade from a layout that kept none.
///
/// One pass over every Entry, each counted into every folder above it.
pub(crate) fn rebuild(connection: &Connection) -> IndexResult<()> {
    const OPERATION: &str = "laying out every folder's totals";
    connection
        .execute("DELETE FROM folder_totals", [])
        .map_err(classify(OPERATION))?;
    let mut folders: BTreeMap<String, Totals> = BTreeMap::new();
    {
        let mut statement = connection
            .prepare(
                "SELECT entries.path, entries.size, entries.mtime,
                     local_entries.path IS NOT NULL
                 FROM entries
                 LEFT JOIN local_entries ON local_entries.path = entries.path
                     AND local_entries.state = ?1 AND local_entries.relocate_to IS NULL",
            )
            .map_err(classify(OPERATION))?;
        let mut entries = statement
            .query(params![rows::state_text(LocalEntryState::Present)])
            .map_err(classify(OPERATION))?;
        while let Some(row) = entries.next().map_err(classify(OPERATION))? {
            let path: String = row.get(0).map_err(classify(OPERATION))?;
            let size: i64 = row.get(1).map_err(classify(OPERATION))?;
            let present: bool = row.get(3).map_err(classify(OPERATION))?;
            let one = Totals {
                files_total: 1,
                files_present: i64::from(present),
                size,
                size_present: if present { size } else { 0 },
                mtime: Some(row.get(2).map_err(classify(OPERATION))?),
            };
            for folder in ancestors(&path) {
                match folders.get_mut(folder) {
                    Some(totals) => totals.add(one),
                    None => {
                        folders.insert(folder.to_owned(), one);
                    }
                }
            }
        }
    }
    for (folder, totals) in &folders {
        write_folder(connection, folder, *totals)?;
    }
    connection
        .execute(
            "UPDATE catalog_counts SET containers = (SELECT count(*) FROM containers)",
            [],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}
