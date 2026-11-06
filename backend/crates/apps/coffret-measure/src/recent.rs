//! The n most recently deposited books.
//!
//! A book is a folder that directly holds files: its pages are the files whose
//! own folder it is, and a folder that holds only folders is not one. A book
//! was deposited when the latest of those files was, so a page added to an
//! old book brings the book forward. The files directly under the Library root
//! count together as one book, the root's own. Two books deposited in the same
//! second are ordered by their folder's path, so the answer is the same every
//! time it is asked.

use std::collections::HashMap;

/// One book, and when it was deposited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentBook {
    /// The folder, as an Entry Path; `None` is the Library root.
    pub folder: Option<String>,
    /// The latest `deposited_at` among the files directly in it, in Unix
    /// seconds.
    pub deposited_at: i64,
}

/// The `n` books deposited last, the latest first, out of every Entry's path
/// and `deposited_at`.
pub fn most_recent_books<'a>(
    entries: impl IntoIterator<Item = (&'a str, i64)>,
    n: usize,
) -> Vec<RecentBook> {
    let mut books: HashMap<Option<&str>, i64> = HashMap::new();
    for (path, deposited_at) in entries {
        let folder = path.rsplit_once('/').map(|(folder, _)| folder);
        books
            .entry(folder)
            .and_modify(|latest| *latest = (*latest).max(deposited_at))
            .or_insert(deposited_at);
    }
    let mut books: Vec<(Option<&str>, i64)> = books.into_iter().collect();
    let newest_first = |a: &(Option<&str>, i64), b: &(Option<&str>, i64)| {
        b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
    };
    if books.len() > n {
        // Only the n newest need ordering among themselves.
        books.select_nth_unstable_by(n, newest_first);
        books.truncate(n);
    }
    books.sort_unstable_by(newest_first);
    books
        .into_iter()
        .map(|(folder, deposited_at)| RecentBook {
            folder: folder.map(str::to_owned),
            deposited_at,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(folder: Option<&str>, deposited_at: i64) -> RecentBook {
        RecentBook {
            folder: folder.map(str::to_owned),
            deposited_at,
        }
    }

    #[test]
    fn a_book_is_a_folder_holding_files_dated_by_its_latest() {
        let entries = [
            ("Comics/Series/Vol. 01/001.jpg", 100),
            ("Comics/Series/Vol. 01/002.jpg", 300),
            ("Comics/Series/Vol. 02/001.jpg", 200),
            ("Novels/Author/Title/001.jpg", 150),
        ];
        assert_eq!(
            most_recent_books(entries, 10),
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
            most_recent_books(entries, 10),
            vec![book(Some("Series/Vol. 01"), 400), book(Some("Series"), 50)],
            "a file deeper down does not date the folder above it"
        );
    }

    #[test]
    fn the_roots_own_files_count_as_one_book() {
        let entries = [
            ("loose-1.jpg", 10),
            ("loose-2.jpg", 500),
            ("loose-3.jpg", 20),
            ("Book/001.jpg", 100),
        ];
        assert_eq!(
            most_recent_books(entries, 10),
            vec![book(None, 500), book(Some("Book"), 100)]
        );
    }

    #[test]
    fn takes_the_n_newest() {
        let entries: Vec<(String, i64)> = (0..100)
            .map(|i| (format!("Shelf/Book {i:03}/001.jpg"), 1_000 + i))
            .collect();
        let recent = most_recent_books(entries.iter().map(|(p, t)| (p.as_str(), *t)), 5);
        let folders: Vec<&str> = recent
            .iter()
            .map(|book| book.folder.as_deref().expect("not the root"))
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
        assert!(most_recent_books(entries.iter().map(|(p, t)| (p.as_str(), *t)), 0).is_empty());
        assert_eq!(
            most_recent_books(entries.iter().map(|(p, t)| (p.as_str(), *t)), 500).len(),
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
            most_recent_books(entries, 2),
            vec![book(Some("A"), 100), book(Some("B"), 100)]
        );
    }
}
