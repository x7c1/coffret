//! The large Library: about a hundred thousand files, to measure against.
//!
//! What `--scenario large` writes, and what `make large-seed`
//! (scripts/large.sh) carries into a Library of its own. The seed is a little
//! of every shape; this is the volume a person's Library reaches after years,
//! in shapes that Library would have: comics in series of volumes, novels
//! under their authors, magazines by title, year and issue, an archive nested
//! five folders deep, and one folder of a few thousand loose scans, which is
//! the folder with the most rows in it. Names are English and Japanese, some
//! long enough to be cut short in a listing, and some written in full-width
//! Latin letters, as a scanner's software sometimes names things.
//!
//! The content is not what is measured, so every page is a tiny JPEG: what the
//! measurements read is the Index and the listing, which a page's bytes do not
//! change. Each page is still drawn from its own position in the sequence, as
//! the benchmark's pages are, rather than copied from one file.
//!
//! The layout is drawn from a fixed pseudo-random sequence, so every run
//! writes the same Library. [`layout`] is the one description of it.
//!
//! The folders to freeze are listed in an order shuffled across the top-level
//! parts, so that the Library's deposit times, which are the moments the
//! freezes commit, do not run part by part: the most recently deposited books
//! are a mix of comics, novels and the rest, as a person's would be.

use crate::{page, save_jpeg, Lcg, Size};
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::BTreeSet;
use std::path::Path;

/// What every page is drawn at: as small as a page drawing still has ink at.
pub const PAGE_SIZE: Size = Size::new(12, 18);

/// One book: a folder of pages, by file name, in reading order.
#[derive(Clone, Debug)]
pub struct Book {
    /// The folder, relative to the output, `/`-separated.
    pub folder: String,
    pub pages: Vec<String>,
}

/// Everything the large Library holds.
#[derive(Clone, Debug, Default)]
pub struct Large {
    pub books: Vec<Book>,
    /// The folders to freeze, one `coffret freeze --under` each, in the order
    /// they are to be frozen.
    pub freeze_units: Vec<String>,
}

impl Large {
    /// How many files there are.
    pub fn files(&self) -> usize {
        self.books.iter().map(|book| book.pages.len()).sum()
    }
}

/// The top-level parts, each mapped under its own name.
pub const COMICS: &str = "Comics";
pub const NOVELS: &str = "Novels";
pub const MAGAZINES: &str = "Magazines";
pub const ARCHIVE: &str = "Archive";
pub const SCANS: &str = "Loose scans";

/// The one series with a fixed name: written in full-width letters, so that a
/// query typed in half-width ones, in either case, finds it only where a
/// match normalizes both.
pub const FULL_WIDTH_SERIES: &str = "ＳＴＡＲ ＬＡＮＴＥＲＮ";

/// How many loose scans the largest folder holds.
pub const SCANS_COUNT: usize = 3000;

const ADJECTIVES: &[&str] = &[
    "Silent",
    "Crimson",
    "Distant",
    "Hidden",
    "Golden",
    "Broken",
    "Endless",
    "Northern",
    "Quiet",
    "Wandering",
    "Iron",
    "Paper",
    "Glass",
    "Midnight",
    "Winter",
    "Summer",
    "Forgotten",
    "Hollow",
    "Scarlet",
    "Velvet",
];

const NOUNS: &[&str] = &[
    "Harbor",
    "Courtyard",
    "Lantern",
    "River",
    "Tower",
    "Orchard",
    "Archive",
    "Voyage",
    "Atlas",
    "Kettle",
    "Meadow",
    "Station",
    "Signal",
    "Quarry",
    "Letters",
    "Island",
    "Bridge",
    "Forest",
    "Clockwork",
    "Valley",
];

const JAPANESE: &[&str] = &[
    "月",
    "海辺",
    "図書館",
    "灯台",
    "銀河",
    "鉄道",
    "夜明け",
    "森",
    "手紙",
    "旅人",
    "時計",
    "硝子",
    "雨",
    "風",
    "星",
    "島",
    "坂道",
    "花火",
    "猫",
    "雪国",
];

const SURNAMES: &[&str] = &[
    "佐藤",
    "鈴木",
    "高橋",
    "田中",
    "伊藤",
    "渡辺",
    "山本",
    "中村",
    "Ashford",
    "Bellamy",
    "Carrow",
    "Duval",
    "Ellery",
    "Fairbanks",
];

const GIVEN_NAMES: &[&str] = &[
    "春樹", "陽子", "健太", "美咲", "直樹", "Margaret", "Thomas", "Eleanor", "Henry", "Clara",
];

const MAGAZINE_TITLES: &[&str] = &[
    "Monthly Railway Review",
    "週刊 写真の世界",
    "The Gardener's Quarterly",
    "月刊 模型工作",
    "Weekly Comic Parade",
    "Illustrated Science Notes",
];

/// What a name runs on with, now and then, so that a listing has to cut it.
const LONG_TAIL: &str =
    ", a title that runs on far longer than any listing has room for, so it is cut short";
const LONG_TAIL_JA: &str = "：一覧の幅に収まりきらず途中で省略されることを確かめるための長い題名";

/// The Library, described.
pub fn layout() -> Large {
    let mut large = Large::default();
    let mut rng = Lcg::new(100_000);
    let mut units: Vec<String> = Vec::new();

    // Comics: series of volumes, a book each.
    let mut names = Names::default();
    for series in 0..60 {
        let name = if series == 0 {
            FULL_WIDTH_SERIES.to_owned()
        } else {
            names.fresh(title(&mut rng, series))
        };
        let folder = format!("{COMICS}/{name}");
        let volumes = 2 + rng.next() as usize % 19;
        for volume in 1..=volumes {
            let pages = 16 + rng.next() as usize % 81;
            large.books.push(Book {
                folder: format!("{folder}/Vol. {volume:02}"),
                pages: pages_of(&mut rng, pages),
            });
        }
        units.push(folder);
    }

    // Novels: titles under their authors, some of them a few hundred pages.
    let mut authors = Names::default();
    for _ in 0..60 {
        let author = authors.fresh(person(&mut rng));
        let folder = format!("{NOVELS}/{author}");
        let mut titles = Names::default();
        let books = 1 + rng.next() as usize % 7;
        for book in 0..books {
            let name = titles.fresh(title(&mut rng, book));
            let pages = if rng.next().is_multiple_of(20) {
                300 + rng.next() as usize % 101
            } else {
                8 + rng.next() as usize % 145
            };
            large.books.push(Book {
                folder: format!("{folder}/{name}"),
                pages: pages_of(&mut rng, pages),
            });
        }
        units.push(folder);
    }

    // Magazines: title, year, issue, three folders down.
    for magazine in MAGAZINE_TITLES {
        for year in 2019..2024 {
            let folder = format!("{MAGAZINES}/{magazine}/{year}");
            for issue in 1..=12 {
                let pages = 16 + rng.next() as usize % 41;
                large.books.push(Book {
                    folder: format!("{folder}/{year}-{issue:02}"),
                    pages: pages_of(&mut rng, pages),
                });
            }
            units.push(folder);
        }
    }

    // An archive nested five folders down to its books.
    for collection in [
        "Family records",
        "家族の記録",
        "Club newsletters",
        "古い写真帳",
    ] {
        let folder = format!("{ARCHIVE}/{collection}");
        for part in 1..=3 {
            for section in ["A", "B", "C", "D"] {
                for book in 1..=6 {
                    let pages = 4 + rng.next() as usize % 53;
                    large.books.push(Book {
                        folder: format!(
                            "{folder}/Part {part}/Section {section}/Box {book}/Bundle {book}"
                        ),
                        pages: pages_of(&mut rng, pages),
                    });
                }
            }
        }
        units.push(folder);
    }

    // The largest folder by rows: a few thousand loose scans of a few pages.
    for scan in 1..=SCANS_COUNT {
        let pages = 1 + rng.next() as usize % 8;
        large.books.push(Book {
            folder: format!("{SCANS}/Scan {scan:04}"),
            pages: pages_of(&mut rng, pages),
        });
    }
    units.push(SCANS.to_owned());

    // Shuffled, so that the deposit times interleave the parts.
    for i in (1..units.len()).rev() {
        let j = rng.next() as usize % (i + 1);
        units.swap(i, j);
    }
    large.freeze_units = units;
    large
}

/// Names not given yet at one level, a repeat told apart by a number.
#[derive(Default)]
struct Names(BTreeSet<String>);

impl Names {
    fn fresh(&mut self, name: String) -> String {
        let mut candidate = name.clone();
        let mut n = 2;
        while !self.0.insert(candidate.clone()) {
            candidate = format!("{name} ({n})");
            n += 1;
        }
        candidate
    }
}

fn pick<'a>(rng: &mut Lcg, words: &[&'a str]) -> &'a str {
    words[rng.next() as usize % words.len()]
}

/// A title: English or Japanese, now and then long, now and then in
/// full-width letters. `nth` varies which, along with the sequence.
fn title(rng: &mut Lcg, nth: usize) -> String {
    let japanese = rng.next().is_multiple_of(3);
    let mut name = if japanese {
        format!("{}の{}", pick(rng, JAPANESE), pick(rng, JAPANESE))
    } else {
        format!("The {} {}", pick(rng, ADJECTIVES), pick(rng, NOUNS))
    };
    if rng.next().is_multiple_of(12) {
        name.push_str(if japanese { LONG_TAIL_JA } else { LONG_TAIL });
    } else if !japanese && (rng.next() as usize + nth).is_multiple_of(10) {
        name = full_width(&name);
    }
    name
}

fn person(rng: &mut Lcg) -> String {
    let surname = pick(rng, SURNAMES);
    let given = pick(rng, GIVEN_NAMES);
    if surname.is_ascii() || given.is_ascii() {
        format!("{given} {surname}")
    } else {
        format!("{surname}{given}")
    }
}

/// ASCII letters and digits in their full-width forms, a space as an
/// ideographic one.
fn full_width(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '!'..='~' => char::from_u32(c as u32 - 0x21 + 0xFF01).unwrap_or(c),
            ' ' => '\u{3000}',
            _ => c,
        })
        .collect()
}

/// `count` page names: numbered, and now and then with a cover before them.
fn pages_of(rng: &mut Lcg, count: usize) -> Vec<String> {
    let width = count.to_string().len().max(3);
    let mut pages: Vec<String> = (1..=count).map(|i| format!("{i:0width$}.jpg")).collect();
    if count > 1 && rng.next().is_multiple_of(7) {
        pages[0] = if rng.next().is_multiple_of(2) {
            "cover.jpg".to_owned()
        } else {
            "表紙.jpg".to_owned()
        };
    }
    pages
}

/// Writes the large Library under `out`, and answers what it wrote.
pub fn generate(out: &Path) -> Result<Large> {
    let large = layout();
    write(out, &large)?;
    Ok(large)
}

/// Writes `large` under `out`.
fn write(out: &Path, large: &Large) -> Result<()> {
    for book in &large.books {
        let folder = out.join(&book.folder);
        std::fs::create_dir_all(&folder)
            .with_context(|| format!("cannot create {}", folder.display()))?;
    }
    let jobs: Vec<(std::path::PathBuf, usize)> = large
        .books
        .iter()
        .flat_map(|book| {
            book.pages
                .iter()
                .map(move |page| out.join(&book.folder).join(page))
        })
        .enumerate()
        .map(|(index, path)| (path, index))
        .collect();
    jobs.par_iter()
        .try_for_each(|(path, index)| save_jpeg(path, page(*index, PAGE_SIZE)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashSet};

    #[test]
    fn holds_about_a_hundred_thousand_files() {
        let files = layout().files();
        assert!(
            (95_000..=105_000).contains(&files),
            "about a hundred thousand files, found {files}"
        );
    }

    #[test]
    fn is_the_same_on_every_run() {
        let (one, two) = (layout(), layout());
        assert_eq!(one.files(), two.files());
        assert_eq!(one.freeze_units, two.freeze_units);
        let folders = |large: &Large| -> Vec<String> {
            large.books.iter().map(|book| book.folder.clone()).collect()
        };
        assert_eq!(folders(&one), folders(&two));
    }

    #[test]
    fn has_the_shapes_it_promises() {
        let large = layout();
        let mut folders = HashSet::new();
        for book in &large.books {
            assert!(
                folders.insert(book.folder.as_str()),
                "{} is one book",
                book.folder
            );
            assert!(!book.pages.is_empty(), "{} has pages", book.folder);
        }
        let longest = large.books.iter().map(|book| book.pages.len()).max();
        assert!(longest >= Some(300), "a book of a few hundred pages");
        assert!(large.books.iter().any(|book| book.pages.len() == 1));

        // Japanese, long and full-width names among them.
        assert!(large.books.iter().any(|book| !book.folder.is_ascii()));
        assert!(large
            .books
            .iter()
            .any(|book| book.folder.contains(LONG_TAIL) || book.folder.contains(LONG_TAIL_JA)));
        assert!(large.books.iter().any(|book| book
            .folder
            .starts_with(&format!("{COMICS}/{FULL_WIDTH_SERIES}/"))));
        assert!(large
            .books
            .iter()
            .any(|book| book.folder.contains('Ｔ') && !book.folder.contains(FULL_WIDTH_SERIES)));

        // Nesting five folders down below a top-level part.
        assert!(large
            .books
            .iter()
            .any(|book| book.folder.starts_with(ARCHIVE) && book.folder.split('/').count() == 6));

        // The loose scans are the folder with the most rows.
        let mut rows: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for book in &large.books {
            let (parent, name) = book.folder.rsplit_once('/').expect("under a part");
            rows.entry(parent).or_default().insert(name);
        }
        let (most, count) = rows
            .iter()
            .map(|(folder, children)| (*folder, children.len()))
            .max_by_key(|(_, count)| *count)
            .expect("folders");
        assert_eq!((most, count), (SCANS, SCANS_COUNT));
    }

    #[test]
    fn freezes_every_book_once_in_an_order_that_mixes_the_parts() {
        let large = layout();
        for book in &large.books {
            let units = large
                .freeze_units
                .iter()
                .filter(|unit| book.folder.starts_with(&format!("{unit}/")))
                .count();
            assert_eq!(units, 1, "{} is frozen once", book.folder);
        }
        let first: BTreeSet<&str> = large
            .freeze_units
            .iter()
            .take(20)
            .map(|unit| unit.split('/').next().expect("a part"))
            .collect();
        assert!(first.len() > 1, "the parts interleave: {first:?}");
    }

    #[test]
    fn writes_every_page_as_a_jpeg() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let large = Large {
            books: vec![
                Book {
                    folder: format!("{COMICS}/{FULL_WIDTH_SERIES}/Vol. 01"),
                    pages: vec!["表紙.jpg".to_owned(), "002.jpg".to_owned()],
                },
                Book {
                    folder: format!("{SCANS}/Scan 0001"),
                    pages: vec!["001.jpg".to_owned()],
                },
            ],
            freeze_units: Vec::new(),
        };
        write(dir.path(), &large).expect("written");
        let cover = dir
            .path()
            .join(COMICS)
            .join(FULL_WIDTH_SERIES)
            .join("Vol. 01/表紙.jpg");
        let image = image::open(&cover).expect("a JPEG");
        assert_eq!(
            (image.width(), image.height()),
            (PAGE_SIZE.width, PAGE_SIZE.height)
        );
        assert!(dir.path().join(SCANS).join("Scan 0001/001.jpg").is_file());
        let other = dir
            .path()
            .join(COMICS)
            .join(FULL_WIDTH_SERIES)
            .join("Vol. 01/002.jpg");
        assert_ne!(
            std::fs::read(&cover).expect("read"),
            std::fs::read(&other).expect("read"),
            "no page is a copy of another"
        );
    }
}
