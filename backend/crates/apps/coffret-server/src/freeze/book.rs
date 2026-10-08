use std::collections::BTreeSet;

use coffret_device::EntryPath;

use crate::folder::Folder;

/// What one freeze is asked to pack: a folder, and where a drop asked for it,
/// exactly the files that drop carried.
///
/// `Book` is this server's name for one freeze's scope — a folder plus an
/// optional selection — and not the concepts' "a book is simply a folder": a
/// `Book` can name some of a folder's files, and loose files dropped at the
/// Library root make one too.
///
/// The folder is what the run is of, as far as anything on a screen goes — the
/// line naming the book, the retry offered under it — and it is the run's
/// prefix. The selection narrows it again, and it is what makes a drop pack what
/// was dropped rather than what happens to be in the folder: a folder the
/// Library already has may hold one-file Entries, and those are eligible for a
/// freeze (spec: PK-1) that nobody who dropped a book beside them asked for. So
/// a drop names its files, and the run considers those and nothing else
/// (spec: PK-17). Naming a file makes nothing eligible that is not: the run
/// still decides that, file by file.
///
/// `None` is every file under the folder, which is what a retry asks for where
/// this server kept no selection to ask it with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    /// The folder the run is of, and its prefix.
    pub folder: Folder,
    /// The Entry Paths to pack, or `None` for everything under the folder.
    pub only: Option<BTreeSet<EntryPath>>,
}

impl Book {
    /// Every eligible file under `folder`.
    pub fn whole(folder: Folder) -> Self {
        Self { folder, only: None }
    }

    /// The files one drop wrote, packed and nothing beside them.
    ///
    /// The folder is the deepest one holding all of them, rather than the one
    /// they were dropped onto: a folder dropped onto `books` is the book, and it
    /// is `books/vol-1` the line names and the folder a reload walks into. Where
    /// the drop carried loose files beside its folders, or several folders, that
    /// is the folder it was dropped onto — and at the Library root it is the
    /// root, which the selection is what keeps from being the whole Library.
    pub fn dropped(written: BTreeSet<EntryPath>) -> Self {
        let mut paths = written.iter();
        let mut holding = paths.next().and_then(EntryPath::parent);
        for path in paths {
            while let Some(folder) = &holding {
                if path.is_under(folder) {
                    break;
                }
                holding = folder.parent();
            }
        }
        Self {
            folder: Folder::named(holding),
            only: Some(written),
        }
    }

    /// Whether a run of this book already packs everything `other` asks for.
    ///
    /// The same folder, and a selection that holds the other's — or no
    /// selection at all, which holds every selection under the folder.
    pub(super) fn covers(&self, other: &Self) -> bool {
        self.folder == other.folder
            && match (&self.only, &other.only) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some(mine), Some(theirs)) => theirs.is_subset(mine),
            }
    }

    /// Takes `other`'s files in as well, for one run that packs both.
    ///
    /// Only for a book of the same folder: two drops into one folder waiting
    /// their turn are one book by the time either is taken up.
    pub(super) fn join(&mut self, other: Self) {
        debug_assert_eq!(self.folder, other.folder);
        match (&mut self.only, other.only) {
            (Some(mine), Some(theirs)) => mine.extend(theirs),
            (only, _) => *only = None,
        }
    }

    /// Whether this asks for every eligible file the mappings reach.
    ///
    /// The one book nothing on this server may arm: the Library root with no
    /// selection is the command line's whole-Library run, and no drop or retry
    /// asks for that (spec: PK-17).
    pub fn is_whole_library(&self) -> bool {
        self.folder.listed().is_none() && self.only.is_none()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::Book;
    use crate::entry_paths::entry_path;
    use crate::folder::Folder;

    fn paths(written: &[&str]) -> BTreeSet<coffret_device::EntryPath> {
        written.iter().map(|path| entry_path(*path)).collect()
    }

    fn folder(path: &str) -> Folder {
        Folder::named((!path.is_empty()).then(|| entry_path(path)))
    }

    // PK-17: what a drop asks to pack is exactly what it wrote — and the folder
    // a screen names it by is the one holding all of it.
    #[test]
    fn a_dropped_book_is_the_paths_written_under_the_folder_holding_them() {
        let written = paths(&["books/vol-1/page-001.png", "books/vol-1/scans/page-002.png"]);
        let book = Book::dropped(written.clone());
        assert_eq!(book.folder, folder("books/vol-1"));
        assert_eq!(book.only, Some(written));
    }

    #[test]
    fn loose_files_beside_a_folder_put_the_book_in_the_folder_dropped_onto() {
        let book = Book::dropped(paths(&["books/vol-1/page-001.png", "books/cover.png"]));
        assert_eq!(book.folder, folder("books"));

        let rooted = Book::dropped(paths(&["vol-1/page-001.png", "vol-2/page-001.png"]));
        assert_eq!(rooted.folder, folder(""));
        assert!(
            !rooted.is_whole_library(),
            "the selection is what keeps a drop at the root from being the whole Library",
        );
    }

    // A name that only starts like the folder is not under it.
    #[test]
    fn a_sibling_sharing_a_prefix_is_not_under_the_folder() {
        let book = Book::dropped(paths(&["books/vol-1/a.png", "books/vol-10/b.png"]));
        assert_eq!(book.folder, folder("books"));
    }

    #[test]
    fn a_book_covers_its_own_files_and_a_whole_folder_covers_every_book_in_it() {
        let dropped = Book::dropped(paths(&["books/vol-1/a.png", "books/vol-1/b.png"]));
        let fewer = Book::dropped(paths(&["books/vol-1/a.png"]));
        let other = Book::dropped(paths(&["books/vol-1/c.png"]));
        assert!(dropped.covers(&fewer));
        assert!(!dropped.covers(&other));
        assert!(!dropped.covers(&Book::whole(folder("books/vol-1"))));
        assert!(Book::whole(folder("books/vol-1")).covers(&dropped));
        assert!(!Book::whole(folder("books")).covers(&dropped));
    }

    #[test]
    fn two_books_of_one_folder_join_into_one_that_packs_both() {
        let mut first = Book::dropped(paths(&["books/vol-1/a.png"]));
        first.join(Book::dropped(paths(&["books/vol-1/b.png"])));
        assert_eq!(
            first.only,
            Some(paths(&["books/vol-1/a.png", "books/vol-1/b.png"]))
        );

        first.join(Book::whole(folder("books/vol-1")));
        assert_eq!(first.only, None, "a whole folder takes every selection in");
    }
}
