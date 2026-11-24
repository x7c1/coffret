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
/// first or last of them where there is no bound (spec: EP-3, LA-32, LA-48).
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
    let read = |row: &rusqlite::Row<'_>| {
        let path: String = row.get(0).map_err(classify(OPERATION))?;
        EntryPath::stored(path).map_err(unreadable_model(OPERATION))
    };
    let sql = statement(children, bound.is_some(), side);
    match bound {
        None => first(connection, &sql, params![folder], OPERATION, read),
        Some(bound) => first(
            connection,
            &sql,
            params![folder, bound.as_str()],
            OPERATION,
            read,
        ),
    }
}

/// The statement [`child_beside`] reads its one row with: `?1` the folder,
/// and `?2` the bound where `bounded` says there is one.
fn statement(children: Children, bounded: bool, side: Side) -> String {
    let (beyond, order) = match side {
        Side::Before => ("<", "DESC"),
        Side::After => (">", "ASC"),
    };
    let table = children.table();
    let bound = if bounded {
        format!(" AND path {beyond} ?2")
    } else {
        String::new()
    };
    format!("SELECT path FROM {table} WHERE parent = ?1{bound} ORDER BY path {order} LIMIT 1")
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::query::plan;

    // LA-48: reading on from a row reads, of each folder it passes, one child
    // folder or Entry through the index on its parent, ended by the first row
    // on the side it looks: never a scan, and never a sort, since the index
    // already holds a folder's children in EP-3 order.
    #[test]
    fn the_child_beside_a_bound_is_one_row_of_an_index() {
        let mut connection = Connection::open_in_memory().expect("an in-memory database opens");
        crate::schema::prepare(&mut connection).expect("a fresh layout lays out");
        for children in [Children::Folders, Children::Entries] {
            for bounded in [false, true] {
                for side in [Side::Before, Side::After] {
                    let sql = statement(children, bounded, side);
                    let steps = plan(&connection, &sql);
                    assert!(
                        steps.iter().all(|step| step.starts_with("SEARCH ")),
                        "{sql} reads by {steps:?}"
                    );
                }
            }
        }
    }
}
