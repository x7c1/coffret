//! Filtering a Library by name and by the most recently deposited books
//! (spec: LA-40, LA-41, LA-42).
//!
//! Out of the Entry Paths and deposit times the catalog holds and nothing
//! else: a Library has no folders to store (spec: EP-2), so a folder matched
//! by name or taken as a book is one the paths imply, and Storage is never
//! asked. It is a full scan of every Entry, in memory, which over a Library of
//! a hundred thousand files is about a tenth of a second — measured, and the
//! reason there is no index of names in the catalog.

mod name_query;
pub use name_query::NameQuery;

mod recent_books;
pub use recent_books::{Books, RecentBook};

use std::num::NonZeroUsize;

use coffret_model::DepositedAt;

/// The most rows a filter answers with (spec: LA-42).
pub const MOST_FILTER_ROWS: usize = 2_000;

/// What a filter asks for: a name, the n most recent books, or both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterAsk {
    name: Option<NameQuery>,
    recent: Option<NonZeroUsize>,
}

impl FilterAsk {
    /// The ask, or `None` where it names neither a name nor a count of books,
    /// which would be no filter at all.
    pub fn new(name: Option<NameQuery>, recent: Option<NonZeroUsize>) -> Option<Self> {
        (name.is_some() || recent.is_some()).then_some(Self { name, recent })
    }
}

/// Whether a row of a filter's answer is a folder or a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilterRowKind {
    /// A folder the Entry Paths beneath it imply (spec: EP-2), or the Library
    /// root as a book.
    Folder,
    /// One Entry.
    File,
}

/// One row a filtered tree keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterRow {
    /// The row's path: an Entry Path for a file, the path a folder's Entries
    /// share for a folder, and the empty string for the Library root, which
    /// is a row only as a book (spec: LA-41).
    pub path: String,
    /// Whether the row is a folder or a file.
    pub kind: FilterRowKind,
    /// For a folder, when it was deposited as a book: the latest deposit time
    /// among the files directly in it, and `None` where it holds none
    /// directly. Always `None` for a file.
    pub deposited_at: Option<DepositedAt>,
}

/// What a filter keeps of a Library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFilter {
    /// The rows matched, in path order, no more than the limit.
    pub rows: Vec<FilterRow>,
    /// How many rows matched over the whole scan, those past the limit
    /// included.
    pub total: usize,
    /// Whether `rows` stops short of `total`.
    pub truncated: bool,
}

/// The rows of a Library that `ask` keeps, out of every Entry's path and
/// deposit time, no more than `limit` of them.
///
/// A name keeps the shallowest folder or file along each Entry Path it stands
/// inside a segment of, and nothing beneath it, nor any folder above it: what
/// a matched folder holds is listed folder by folder as it is opened, and the
/// folders above are what its path already says (spec: LA-40). The n most
/// recent books are the folders directly holding the files deposited last,
/// the root counting as one (spec: LA-41). Both together are the n most recent
/// books whose own path a name stands inside a segment of: the books named by
/// a match or under one (spec: LA-42).
pub fn filter_library(
    entries: &[(&str, DepositedAt)],
    ask: &FilterAsk,
    limit: usize,
) -> LibraryFilter {
    let books = Books::of(entries.iter().copied());
    let mut rows: Vec<(&str, FilterRowKind)> = match (&ask.name, ask.recent) {
        (name, Some(n)) => books
            .most_recent(n.get(), |folder| match name {
                None => true,
                // The root's own files are a book with no path, which no name
                // stands inside.
                Some(name) => folder.is_some_and(|folder| name.matches(folder)),
            })
            .into_iter()
            .map(|book| (book.folder.unwrap_or(""), FilterRowKind::Folder))
            .collect(),
        (Some(name), None) => entries
            .iter()
            .filter_map(|(path, _)| {
                name.shallowest_match(path).map(|end| {
                    let kind = if end == path.len() {
                        FilterRowKind::File
                    } else {
                        FilterRowKind::Folder
                    };
                    (&path[..end], kind)
                })
            })
            .collect(),
        (None, None) => unreachable!("an ask names a name, a count of books, or both"),
    };
    rows.sort_unstable();
    rows.dedup();

    let total = rows.len();
    rows.truncate(limit);
    LibraryFilter {
        rows: rows
            .into_iter()
            .map(|(path, kind)| FilterRow {
                path: path.to_owned(),
                kind,
                deposited_at: match kind {
                    FilterRowKind::Folder => books.deposited_at((!path.is_empty()).then_some(path)),
                    FilterRowKind::File => None,
                },
            })
            .collect(),
        total,
        truncated: total > limit,
    }
}

#[cfg(test)]
mod tests;
