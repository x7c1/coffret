use coffret_model::EntryPath;
use coffret_usecase::folder_figures::Side;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use super::ROOT;
use crate::error::{classify, unreadable_model};
use crate::query::first;

/// Where a lookup beside a bound reads its one row from: the folders of the
/// Library or its Entries, each keyed by the folder it stands directly in.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Children {
    /// `folder_totals`, through its index on `(parent, path)`.
    Folders,
    /// `entries`, through its index on `(parent, path)`.
    Entries,
}

impl Children {
    fn table(self) -> &'static str {
        match self {
            Self::Folders => "folder_totals",
            Self::Entries => "entries",
        }
    }
}

/// The nearest child of `folder` of one kind on `side` of `bound`, or the
/// first or last of them where there is no bound (spec: EP-3, LA-32).
///
/// One row through the `(parent, path)` index, bounded by `LIMIT 1`: siblings
/// share their parent, so the index walks them in EP-3 order, which is the
/// byte order of their whole paths, and the first row on the right side of
/// the bound is the answer.
pub(crate) fn child_beside(
    connection: &Connection,
    children: Children,
    folder: Option<&EntryPath>,
    bound: Option<&EntryPath>,
    side: Side,
) -> IndexResult<Option<EntryPath>> {
    const OPERATION: &str = "reading the nearest child of a folder";
    let folder = folder.map_or(ROOT, EntryPath::as_str);
    let (beyond, order) = match side {
        Side::Before => ("<", "DESC"),
        Side::After => (">", "ASC"),
    };
    let table = children.table();
    let read = |row: &rusqlite::Row<'_>| {
        let path: String = row.get(0).map_err(classify(OPERATION))?;
        EntryPath::stored(path).map_err(unreadable_model(OPERATION))
    };
    match bound {
        None => first(
            connection,
            &format!("SELECT path FROM {table} WHERE parent = ?1 ORDER BY path {order} LIMIT 1"),
            params![folder],
            OPERATION,
            read,
        ),
        Some(bound) => first(
            connection,
            &format!(
                "SELECT path FROM {table} WHERE parent = ?1 AND path {beyond} ?2 \
                 ORDER BY path {order} LIMIT 1"
            ),
            params![folder, bound.as_str()],
            OPERATION,
            read,
        ),
    }
}
