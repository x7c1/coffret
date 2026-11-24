//! What each folder of the Library comes to, kept beside the catalog so that a
//! listing reads it rather than the subtree under the folder (spec: LA-45).
//!
//! `folder_totals` holds one row per folder with a current Entry under it at
//! any depth, the Library root included as the row whose path is empty: how
//! many Entries stand below it, how many of them this device has by its
//! present materialization records (spec: EP-10), what those Entries and the
//! present ones come to in bytes, and the newest `mtime` among them.
//! `catalog_counts` holds the one count no folder has, the current
//! Containers.
//!
//! Both are a derivation of this file's own rows — the catalog's Entries and
//! this device's records of what it has — and so neither is carried by an Index
//! Snapshot nor reaches Storage (spec: CK-7). They are kept up to date by the
//! writes themselves rather than by triggers: every operation that changes the
//! Entries or the materialization records hands [`recount`] the Entry Paths it
//! touched, in the same transaction as its writes, and a restore, which
//! replaces the whole catalog, has [`rebuild`] lay the figures out afresh.
//!
//! A recount takes each folder above a touched path from the bottom up, and
//! counts it out of what stands directly in it: the Entries whose `parent` is
//! the folder, read through the index on that column, and the rows of its
//! child folders, already recounted. Nothing below the children is read, so a
//! write costs each folder above it its own rows, never its subtree.

use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::classify;

mod recount;
pub(crate) use recount::recount;

mod rebuild;
pub(crate) use rebuild::rebuild;

mod count_containers;
pub(crate) use count_containers::count_containers;

mod folder_contents;
pub(crate) use folder_contents::folder_contents;

mod library_summary;
pub(crate) use library_summary::library_summary;

mod folders;
pub(crate) use folders::folders;

mod child_beside;
pub(crate) use child_beside::{child_beside, Children};

/// The path `folder_totals` holds the Library root's row under, and the
/// `parent` of an Entry or a folder standing directly in the root. No Entry
/// Path is empty (spec: EP-2), so it names nothing else.
pub(crate) const ROOT: &str = "";

/// The folder an Entry or a folder at `path` stands directly in, as the
/// `parent` columns spell it: what is left of the path once its last
/// component comes off, or [`ROOT`] for a path of one component.
///
/// Cut out of the text at the last separator, which is the only logical
/// separator an Entry Path has (spec: EP-2).
pub(crate) fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(ROOT, |(parent, _)| parent)
}

/// Every folder `path` stands under, nearest first and the root last.
pub(super) fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    let mut next = (path != ROOT).then_some(path);
    std::iter::from_fn(move || {
        let at = next?;
        let parent = parent_of(at);
        next = (parent != ROOT).then_some(parent);
        Some(parent)
    })
}

/// How many components deep a folder stands, the root being none.
pub(super) fn depth(folder: &str) -> usize {
    match folder {
        ROOT => 0,
        folder => folder.bytes().filter(|&byte| byte == b'/').count() + 1,
    }
}

/// What one folder comes to, counted out of the Entries standing directly in
/// it and the rows of its child folders.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Totals {
    pub(super) files_total: i64,
    pub(super) files_present: i64,
    pub(super) size: i64,
    pub(super) size_present: i64,
    pub(super) mtime: Option<i64>,
}

impl Totals {
    pub(super) fn add(&mut self, other: Totals) {
        self.files_total += other.files_total;
        self.files_present += other.files_present;
        self.size += other.size;
        self.size_present += other.size_present;
        self.mtime = self.mtime.max(other.mtime);
    }
}

/// Writes one folder's row, or takes it out where nothing stands under the
/// folder any more.
pub(super) fn write_folder(
    connection: &Connection,
    folder: &str,
    totals: Totals,
) -> IndexResult<()> {
    const OPERATION: &str = "writing a folder's totals";
    let Some(mtime) = totals.mtime.filter(|_| totals.files_total > 0) else {
        connection
            .prepare_cached("DELETE FROM folder_totals WHERE path = ?1")
            .and_then(|mut statement| statement.execute(params![folder]))
            .map_err(classify(OPERATION))?;
        return Ok(());
    };
    let parent = (folder != ROOT).then(|| parent_of(folder));
    connection
        .prepare_cached(
            "INSERT INTO folder_totals (
                 path, parent, files_total, files_present, size, size_present, mtime
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (path) DO UPDATE SET
                 files_total = excluded.files_total,
                 files_present = excluded.files_present,
                 size = excluded.size,
                 size_present = excluded.size_present,
                 mtime = excluded.mtime",
        )
        .and_then(|mut statement| {
            statement.execute(params![
                folder,
                parent,
                totals.files_total,
                totals.files_present,
                totals.size,
                totals.size_present,
                mtime,
            ])
        })
        .map_err(classify(OPERATION))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::folder_contents::{CHILD_FOLDERS, FILES};
    use super::folders::FOLDERS;
    use super::library_summary::LIBRARY;
    use super::recount::{COUNT_CHILD_FOLDERS, COUNT_FILES};
    use super::*;
    use crate::query::plan;

    // LA-45: what a listing, the Library's totals and the list of folders read
    // is the folder's own rows, each found through an index, and never a scan
    // of the Entries, the materialization records or the Containers; nor a
    // sort, since each index already holds its rows in EP-3 order. A write's
    // recount of one folder reads its own rows the same way.
    #[test]
    fn a_folder_is_read_by_its_own_rows_and_never_by_a_scan() {
        let mut connection = Connection::open_in_memory().expect("an in-memory database opens");
        crate::schema::prepare(&mut connection).expect("a fresh layout lays out");
        for sql in [
            CHILD_FOLDERS,
            FILES,
            LIBRARY,
            FOLDERS,
            COUNT_FILES,
            COUNT_CHILD_FOLDERS,
        ] {
            let steps = plan(&connection, sql);
            for step in &steps {
                assert!(
                    step.starts_with("SEARCH ") || step == "SCAN catalog_counts",
                    "{sql} reads by {step:?}, in {steps:?}"
                );
            }
        }
    }

    // The catalog's checks cut a path's folder out in SQL; they must cut the
    // one `parent_of` does, or a write this module makes is refused.
    #[test]
    fn the_schema_cuts_a_folder_out_of_a_path_as_parent_of_does() {
        let connection = Connection::open_in_memory().expect("an in-memory database opens");
        for path in [
            "notes.txt",
            "albums/2026/spring.jpg",
            "a/a",
            "\u{6f2b}\u{753b}/\u{7b2c}1\u{5dfb}/001.png",
            "x.y/z",
        ] {
            let cut: String = connection
                .query_row(
                    "SELECT rtrim(rtrim(?1, replace(?1, '/', '')), '/')",
                    [path],
                    |row| row.get(0),
                )
                .expect("the expression evaluates");
            assert_eq!(cut, parent_of(path), "{path}");
        }
    }

    #[test]
    fn a_path_stands_under_every_prefix_of_it_and_the_root() {
        assert_eq!(parent_of("albums/2026/spring.jpg"), "albums/2026");
        assert_eq!(parent_of("notes.txt"), ROOT);
        assert_eq!(
            ancestors("albums/2026/spring.jpg").collect::<Vec<_>>(),
            ["albums/2026", "albums", ROOT]
        );
        assert_eq!(ancestors("notes.txt").collect::<Vec<_>>(), [ROOT]);
        assert_eq!(depth(ROOT), 0);
        assert_eq!(depth("albums"), 1);
        assert_eq!(depth("albums/2026"), 2);
    }
}
