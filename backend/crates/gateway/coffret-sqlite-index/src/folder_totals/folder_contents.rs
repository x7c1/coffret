use coffret_model::EntryPath;
use coffret_usecase::device_state::LocalEntryState;
use coffret_usecase::folder_figures::FolderContents;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use super::ROOT;
use crate::error::classify;
use crate::query::collect;
use crate::rows;

/// One folder's child folders' rows, in EP-3 order.
pub(super) const CHILD_FOLDERS: &str =
    "SELECT * FROM folder_totals WHERE parent = ?1 ORDER BY path";

/// The Entries standing directly in one folder, in EP-3 order, with whether
/// this device has each and the kind of its Container.
pub(super) const FILES: &str = "SELECT entries.*, local_entries.path IS NOT NULL AS present,
         containers.kind AS kind
     FROM entries
     LEFT JOIN local_entries ON local_entries.path = entries.path
         AND local_entries.state = ?2 AND local_entries.relocate_to IS NULL
     LEFT JOIN containers ON containers.id = entries.container_id
     WHERE entries.parent = ?1
     ORDER BY entries.path";

/// What one folder holds, one level down (spec: LA-44, LA-45): its child
/// folders' rows, and the Entries whose `parent` is the folder with whether
/// this device has each and the kind of its Container.
///
/// Two statements in one read transaction, so both halves are of one moment
/// of the catalog even where another process commits between them
/// (spec: CK-13).
pub(crate) fn folder_contents(
    connection: &Connection,
    folder: Option<&EntryPath>,
) -> IndexResult<FolderContents> {
    const OPERATION: &str = "reading one folder's contents";
    let folder = folder.map_or(ROOT, EntryPath::as_str);
    let transaction = connection
        .unchecked_transaction()
        .map_err(classify(OPERATION))?;
    let folders = collect(
        &transaction,
        CHILD_FOLDERS,
        params![folder],
        OPERATION,
        rows::folder_summary,
    )?;
    let files = collect(
        &transaction,
        FILES,
        params![folder, rows::state_text(LocalEntryState::Present)],
        OPERATION,
        rows::folder_file,
    )?;
    transaction.commit().map_err(classify(OPERATION))?;
    Ok(FolderContents { folders, files })
}
