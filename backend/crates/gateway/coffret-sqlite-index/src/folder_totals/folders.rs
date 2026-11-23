use coffret_model::EntryPath;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use super::ROOT;
use crate::error::classify;
use crate::query::collect;

/// Every row but the root's, in EP-3 order.
pub(super) const FOLDERS: &str = "SELECT path FROM folder_totals WHERE path > ?1 ORDER BY path";

/// Every folder of the Library, in EP-3 order (spec: EP-3, LA-45): each row
/// but the root's, read off the primary key in its own order.
pub(crate) fn folders(connection: &Connection) -> IndexResult<Vec<EntryPath>> {
    const OPERATION: &str = "reading the Library's folders";
    collect(connection, FOLDERS, params![ROOT], OPERATION, |row| {
        let path: String = row.get(0).map_err(classify(OPERATION))?;
        EntryPath::stored(path).map_err(crate::error::unreadable_model(OPERATION))
    })
}
