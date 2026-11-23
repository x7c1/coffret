use coffret_usecase::folder_figures::LibrarySummary;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use super::ROOT;
use crate::query::first;
use crate::rows;

/// The root's row and the count of current Containers, as one row whatever
/// the Library holds.
pub(super) const LIBRARY: &str = "SELECT ifnull(folder_totals.files_total, 0) AS files_total,
         ifnull(folder_totals.files_present, 0) AS files_present,
         ifnull(folder_totals.size, 0) AS size,
         ifnull(folder_totals.size_present, 0) AS size_present,
         folder_totals.mtime AS mtime,
         catalog_counts.containers AS containers
     FROM catalog_counts
     LEFT JOIN folder_totals ON folder_totals.path = ?1";

/// What the whole Library comes to: the root's row, and the count of current
/// Containers (spec: LA-39, LA-45).
pub(crate) fn library_summary(connection: &Connection) -> IndexResult<LibrarySummary> {
    Ok(first(
        connection,
        LIBRARY,
        params![ROOT],
        "reading the Library's totals",
        rows::library_summary,
    )?
    .unwrap_or_default())
}
