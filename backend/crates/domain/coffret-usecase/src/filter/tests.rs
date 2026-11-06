use std::num::NonZeroUsize;

use coffret_model::DepositedAt;

use super::{filter_library, FilterAsk, FilterRow, FilterRowKind, LibraryFilter, NameQuery};

const LIBRARY: [(&str, i64); 8] = [
    ("Comics/Starfall/Vol. 01/001.jpg", 100),
    ("Comics/Starfall/Vol. 01/002.jpg", 400),
    ("Comics/Starfall/Vol. 02/001.jpg", 200),
    ("Comics/Other/Vol. 01/starfall-poster.jpg", 500),
    ("Novels/Starfall Diaries/001.jpg", 300),
    ("Novels/Quiet/001.jpg", 600),
    ("loose.jpg", 700),
    ("starfall.txt", 50),
];

fn filtered(name: Option<&str>, recent: Option<usize>, limit: usize) -> LibraryFilter {
    let entries: Vec<(&str, DepositedAt)> = LIBRARY
        .iter()
        .map(|(path, seconds)| (*path, DepositedAt::from_unix_seconds(*seconds)))
        .collect();
    let ask = FilterAsk::new(
        name.map(|typed| NameQuery::new(typed).expect("a name")),
        recent.map(|n| NonZeroUsize::new(n).expect("a positive count")),
    )
    .expect("an ask");
    filter_library(&entries, &ask, limit)
}

fn folder(path: &str, seconds: Option<i64>) -> FilterRow {
    FilterRow {
        path: path.to_owned(),
        kind: FilterRowKind::Folder,
        deposited_at: seconds.map(DepositedAt::from_unix_seconds),
    }
}

fn file(path: &str) -> FilterRow {
    FilterRow {
        path: path.to_owned(),
        kind: FilterRowKind::File,
        deposited_at: None,
    }
}

// LA-40: a name keeps the shallowest folder or file it matches along each
// path — never what a matched folder holds, nor the folders above a match.
#[test]
fn a_name_keeps_the_shallowest_match_along_each_path() {
    let answer = filtered(Some("starfall"), None, 100);
    assert_eq!(
        answer.rows,
        vec![
            file("Comics/Other/Vol. 01/starfall-poster.jpg"),
            folder("Comics/Starfall", None),
            folder("Novels/Starfall Diaries", Some(300)),
            file("starfall.txt"),
        ],
    );
    assert_eq!((answer.total, answer.truncated), (4, false));
}

#[test]
fn a_name_that_matches_nothing_keeps_nothing() {
    let answer = filtered(Some("harbor"), None, 100);
    assert!(answer.rows.is_empty());
    assert_eq!((answer.total, answer.truncated), (0, false));
}

// LA-41: the n most recent books, the root's own files counting as one, and
// in path order rather than by time.
#[test]
fn the_most_recent_books_are_answered_in_path_order() {
    let answer = filtered(None, Some(3), 100);
    assert_eq!(
        answer.rows,
        vec![
            folder("", Some(700)),
            folder("Comics/Other/Vol. 01", Some(500)),
            folder("Novels/Quiet", Some(600)),
        ],
    );
    assert_eq!(answer.total, 3);
}

// LA-42: both together are the newest books named by a match or under one.
// The root, and a book holding a matched file only, are neither.
#[test]
fn a_name_and_a_count_take_the_newest_books_within_the_match() {
    let answer = filtered(Some("starfall"), Some(2), 100);
    assert_eq!(
        answer.rows,
        vec![
            folder("Comics/Starfall/Vol. 01", Some(400)),
            folder("Novels/Starfall Diaries", Some(300)),
        ],
    );
    let all = filtered(Some("starfall"), Some(10), 100);
    assert_eq!(all.total, 3, "{all:?}");
}

// LA-42: past the limit the rows stop, in path order, and the total counts
// every match.
#[test]
fn rows_past_the_limit_are_counted_and_not_answered() {
    let answer = filtered(Some("jpg"), None, 2);
    assert_eq!(
        answer.rows,
        vec![
            file("Comics/Other/Vol. 01/starfall-poster.jpg"),
            file("Comics/Starfall/Vol. 01/001.jpg"),
        ],
    );
    assert_eq!((answer.total, answer.truncated), (7, true));
}

#[test]
fn an_ask_names_something() {
    assert_eq!(FilterAsk::new(None, None), None);
}
