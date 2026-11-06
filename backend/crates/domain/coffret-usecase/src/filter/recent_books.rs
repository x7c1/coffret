use std::collections::HashMap;

use coffret_model::DepositedAt;

/// Every book in a Library, and when each was deposited (spec: LA-41).
///
/// A book is a folder that directly holds files: its pages are the files whose
/// own folder it is, and a folder that holds only folders — a series — is not
/// one. A book was deposited when the latest of those files was, so a page
/// added to an old book brings the book forward, and a rename or a move, which
/// carries each file's deposit time with it (spec: FM-15, FM-16), leaves the
/// order as it was. The files directly under the Library root count together
/// as one book, the root's own, which is the folder `None`.
#[derive(Debug, Default)]
pub struct Books<'a> {
    deposited: HashMap<Option<&'a str>, DepositedAt>,
}

impl<'a> Books<'a> {
    /// The books out of every Entry's path and deposit time.
    pub fn of(entries: impl IntoIterator<Item = (&'a str, DepositedAt)>) -> Self {
        let mut deposited: HashMap<Option<&'a str>, DepositedAt> = HashMap::new();
        for (path, deposited_at) in entries {
            let folder = path.rsplit_once('/').map(|(folder, _)| folder);
            deposited
                .entry(folder)
                .and_modify(|latest| *latest = (*latest).max(deposited_at))
                .or_insert(deposited_at);
        }
        Self { deposited }
    }

    /// When the book `folder` is was deposited, and `None` where the folder
    /// holds no file directly, which makes it no book.
    pub fn deposited_at(&self, folder: Option<&str>) -> Option<DepositedAt> {
        self.deposited.get(&folder).copied()
    }

    /// The `n` books deposited last that `kept` keeps, the latest first.
    ///
    /// Two books deposited in the same second are ordered by their folder's
    /// path, the root first, so the answer is the same every time it is asked.
    pub fn most_recent(
        &self,
        n: usize,
        kept: impl Fn(Option<&str>) -> bool,
    ) -> Vec<RecentBook<'a>> {
        let mut books: Vec<RecentBook<'a>> = self
            .deposited
            .iter()
            .filter(|(folder, _)| kept(**folder))
            .map(|(folder, deposited_at)| RecentBook {
                folder: *folder,
                deposited_at: *deposited_at,
            })
            .collect();
        let newest_first = |a: &RecentBook<'_>, b: &RecentBook<'_>| {
            b.deposited_at
                .cmp(&a.deposited_at)
                .then_with(|| a.folder.cmp(&b.folder))
        };
        if books.len() > n {
            // Only the n newest need ordering among themselves.
            books.select_nth_unstable_by(n, newest_first);
            books.truncate(n);
        }
        books.sort_unstable_by(newest_first);
        books
    }
}

/// One book, and when it was deposited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecentBook<'a> {
    /// The folder, as an Entry Path; `None` is the Library root.
    pub folder: Option<&'a str>,
    /// The latest deposit time among the files directly in it.
    pub deposited_at: DepositedAt,
}

#[cfg(test)]
mod tests {
    use super::{Books, RecentBook};
    use coffret_model::DepositedAt;

    fn at(seconds: i64) -> DepositedAt {
        DepositedAt::from_unix_seconds(seconds)
    }

    fn book(folder: Option<&str>, seconds: i64) -> RecentBook<'_> {
        RecentBook {
            folder,
            deposited_at: at(seconds),
        }
    }

    fn most_recent<'a>(entries: &[(&'a str, i64)], n: usize) -> Vec<RecentBook<'a>> {
        Books::of(entries.iter().map(|(path, seconds)| (*path, at(*seconds))))
            .most_recent(n, |_| true)
    }

    // LA-41: a book is a folder holding files, dated by the latest of them; a
    // series holding only folders is not one.
    #[test]
    fn a_book_is_a_folder_holding_files_dated_by_its_latest() {
        let entries = [
            ("Comics/Series/Vol. 01/001.jpg", 100),
            ("Comics/Series/Vol. 01/002.jpg", 300),
            ("Comics/Series/Vol. 02/001.jpg", 200),
            ("Novels/Author/Title/001.jpg", 150),
        ];
        assert_eq!(
            most_recent(&entries, 10),
            vec![
                book(Some("Comics/Series/Vol. 01"), 300),
                book(Some("Comics/Series/Vol. 02"), 200),
                book(Some("Novels/Author/Title"), 150),
            ],
            "the folders holding only folders, `Comics/Series` and the rest, are not books"
        );
    }

    #[test]
    fn a_folder_holding_files_and_folders_is_a_book_of_its_own_files() {
        let entries = [("Series/notes.txt", 50), ("Series/Vol. 01/001.jpg", 400)];
        assert_eq!(
            most_recent(&entries, 10),
            vec![book(Some("Series/Vol. 01"), 400), book(Some("Series"), 50)],
            "a file deeper down does not date the folder above it"
        );
    }

    // LA-41: the files directly under the root count as one book.
    #[test]
    fn the_roots_own_files_count_as_one_book() {
        let entries = [
            ("loose-1.jpg", 10),
            ("loose-2.jpg", 500),
            ("loose-3.jpg", 20),
            ("Book/001.jpg", 100),
        ];
        assert_eq!(
            most_recent(&entries, 10),
            vec![book(None, 500), book(Some("Book"), 100)]
        );
    }

    // LA-41: the n newest are taken.
    #[test]
    fn takes_the_n_newest() {
        let paths: Vec<String> = (0..100)
            .map(|i| format!("Shelf/Book {i:03}/001.jpg"))
            .collect();
        let entries: Vec<(&str, i64)> = paths
            .iter()
            .enumerate()
            .map(|(i, path)| (path.as_str(), 1_000 + i as i64))
            .collect();
        let folders: Vec<&str> = most_recent(&entries, 5)
            .iter()
            .map(|book| book.folder.expect("not the root"))
            .collect();
        assert_eq!(
            folders,
            [
                "Shelf/Book 099",
                "Shelf/Book 098",
                "Shelf/Book 097",
                "Shelf/Book 096",
                "Shelf/Book 095"
            ]
        );
        assert!(most_recent(&entries, 0).is_empty());
        assert_eq!(
            most_recent(&entries, 500).len(),
            100,
            "fewer books than asked for are all of them"
        );
    }

    #[test]
    fn ties_are_ordered_by_folder() {
        let entries = [
            ("B/001.jpg", 100),
            ("A/001.jpg", 100),
            ("C/001.jpg", 100),
            ("D/001.jpg", 50),
        ];
        assert_eq!(
            most_recent(&entries, 2),
            vec![book(Some("A"), 100), book(Some("B"), 100)]
        );
    }

    #[test]
    fn only_the_books_kept_are_taken() {
        let entries = [("A/001.jpg", 300), ("B/001.jpg", 200), ("C/001.jpg", 100)];
        let books = Books::of(entries.iter().map(|(path, seconds)| (*path, at(*seconds))));
        assert_eq!(
            books.most_recent(1, |folder| folder != Some("A")),
            vec![book(Some("B"), 200)],
        );
        assert_eq!(books.deposited_at(Some("C")), Some(at(100)));
        assert_eq!(books.deposited_at(None), None, "the root holds no file");
    }
}
