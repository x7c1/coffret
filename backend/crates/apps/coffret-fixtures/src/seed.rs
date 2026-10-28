//! The seed: a Library to look at while working on the explorer.
//!
//! What `--scenario seed` writes, and what `make local` (scripts/local.sh)
//! carries into the Library it serves. The benchmark's output is volume of one
//! shape; this is the opposite, a little of every shape a listing or the reader
//! has to get right, at sizes small enough that the whole of it is generated,
//! encrypted and uploaded in a minute or two.
//!
//! [`layout`] below is the one description of it. A pattern worth having in
//! the Library — a kind of name, a shape of book, a depth of folder — is added
//! there, as one more call, and the test at the bottom of this file says what
//! the seed is promised to hold.
//!
//! Every directory directly under the output is a top-level part of the
//! Library: the script maps each one with `map --prefix` under its own name.
//! Every book is a folder of page images, and every page shows its position in
//! its book — the order a person reads it in, from 1 — in red digits, so that a
//! page shown out of order says so at a glance. A book is carried in by a
//! freeze, as a book dropped onto a folder in the explorer is, and the folders
//! to freeze are what [`Seed::freeze_units`] lists: each book on its own,
//! except where a shelf of many books is frozen in one run.

use crate::{page, photo, save_jpeg, Size};
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

/// The image sizes the seed is drawn at.
#[derive(Clone, Copy, Debug)]
pub struct Sizes {
    pub portrait: Size,
    pub landscape: Size,
    pub spread: Size,
    pub photo: Size,
}

impl Sizes {
    /// What `--scenario seed` draws at: large enough for the digits and the
    /// shape of a page to read in the reader, small enough to keep the seed's
    /// upload quick.
    pub const SMALL: Self = Self {
        portrait: Size::new(400, 600),
        landscape: Size::new(600, 400),
        spread: Size::new(800, 600),
        photo: Size::new(480, 360),
    };
}

/// The shape of one page image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Portrait,
    Landscape,
    /// Two portrait pages side by side, as a scanner that takes an open book
    /// in one image makes them.
    Spread,
}

/// One page image of a book, under its file name.
#[derive(Clone, Debug)]
pub struct Page {
    pub name: String,
    pub shape: Shape,
}

/// One book: a folder of pages, listed in the order a person reads them.
#[derive(Clone, Debug)]
pub struct Book {
    /// The folder, relative to the output, `/`-separated.
    pub folder: String,
    pub pages: Vec<Page>,
}

/// One photo album: a folder of photos.
#[derive(Clone, Debug)]
pub struct Album {
    pub folder: String,
    pub photos: usize,
}

/// Everything the seed holds.
#[derive(Clone, Debug, Default)]
pub struct Seed {
    pub books: Vec<Book>,
    pub albums: Vec<Album>,
    /// Folders made and left empty.
    pub empty: Vec<String>,
    /// The folders to freeze, one `coffret freeze --under` each.
    pub freeze_units: Vec<String>,
}

impl Seed {
    /// A book, frozen on its own.
    fn book(&mut self, folder: &str, pages: Vec<Page>) {
        self.freeze_units.push(folder.to_owned());
        self.shelved(folder, pages);
    }

    /// A book frozen with the rest of its shelf rather than on its own.
    fn shelved(&mut self, folder: &str, pages: Vec<Page>) {
        self.books.push(Book {
            folder: folder.to_owned(),
            pages,
        });
    }

    /// A folder holding `count` books of `pages` pages each, frozen in one run:
    /// hundreds of freezes of their own would take minutes for what one does
    /// in seconds.
    fn shelf(&mut self, folder: &str, count: usize, pages: usize) {
        self.freeze_units.push(folder.to_owned());
        for i in 1..=count {
            self.shelved(
                &format!("{folder}/Book {i:03}"),
                numbered(pages, Shape::Portrait),
            );
        }
    }

    fn album(&mut self, folder: &str, photos: usize) {
        self.albums.push(Album {
            folder: folder.to_owned(),
            photos,
        });
    }

    fn empty(&mut self, folder: &str) {
        self.empty.push(folder.to_owned());
    }
}

/// `count` pages named in sequence, zero-padded to three digits at least:
/// `001.jpg`, `002.jpg`, and on.
fn numbered(count: usize, shape: Shape) -> Vec<Page> {
    let width = count.to_string().len().max(3);
    (1..=count)
        .map(|i| Page {
            name: format!("{i:0width$}.jpg"),
            shape,
        })
        .collect()
}

/// Pages named as they are listed, all portrait.
fn named(names: &[&str]) -> Vec<Page> {
    names
        .iter()
        .map(|name| Page {
            name: (*name).to_owned(),
            shape: Shape::Portrait,
        })
        .collect()
}

/// The top-level parts of the Library.
pub const BOOKS: &str = "Books";
pub const ALBUMS: &str = "Albums";
/// Mapped and left empty, so the root lists a top-level folder with nothing in
/// it yet, to drop into. It is the only empty folder the seed has: a folder of
/// the Library is what the paths under it imply, so an empty directory deeper
/// down is carried in as nothing and never shows in the explorer.
pub const EMPTY: &str = "Nothing here yet";

/// The seed, described: add a pattern here.
pub fn layout() -> Seed {
    let mut seed = Seed::default();

    // Books of different shapes.
    seed.book(
        &format!("{BOOKS}/A single page"),
        numbered(1, Shape::Portrait),
    );
    seed.book(
        &format!("{BOOKS}/A short book"),
        numbered(10, Shape::Portrait),
    );
    seed.book(
        &format!("{BOOKS}/A long book"),
        numbered(300, Shape::Portrait),
    );
    seed.book(
        &format!("{BOOKS}/Landscape pages"),
        numbered(12, Shape::Landscape),
    );
    // A single first page, spreads after it, and a single last page: how a
    // scanned book whose cover was scanned on its own comes out.
    let mut spreads = vec![Page {
        name: "001.jpg".to_owned(),
        shape: Shape::Portrait,
    }];
    for i in 0..6 {
        let left = 2 + 2 * i;
        spreads.push(Page {
            name: format!("{left:03}-{:03}.jpg", left + 1),
            shape: Shape::Spread,
        });
    }
    spreads.push(Page {
        name: "014.jpg".to_owned(),
        shape: Shape::Portrait,
    });
    seed.book(&format!("{BOOKS}/Single pages and spreads"), spreads);

    // Names that sort differently as text and as numbers: p1, p10, p11, p12,
    // p2, … as text.
    let unpadded: Vec<String> = (1..=12).map(|i| format!("p{i}.jpg")).collect();
    let unpadded: Vec<&str> = unpadded.iter().map(String::as_str).collect();
    seed.book(&format!("{BOOKS}/Pages out of sequence"), named(&unpadded));

    // Numbered pages with unnumbered ones before and after them.
    let mut matter = vec!["cover.jpg", "contents.jpg"];
    let body: Vec<String> = (1..=8).map(|i| format!("{i:03}.jpg")).collect();
    matter.extend(body.iter().map(String::as_str));
    matter.extend(["afterword.jpg", "back.jpg"]);
    seed.book(&format!("{BOOKS}/A cover and back matter"), named(&matter));

    // Names: Japanese ones, and ones too long for a listing to show whole.
    seed.book(
        &format!("{BOOKS}/吾輩は猫である"),
        named(&[
            "表紙.jpg",
            "001.jpg",
            "002.jpg",
            "003.jpg",
            "004.jpg",
            "奥付.jpg",
        ]),
    );
    seed.book(
        &format!("{BOOKS}/日本の作家/夏目漱石/こころ"),
        numbered(8, Shape::Portrait),
    );
    seed.book(
        &format!(
            "{BOOKS}/A book whose title runs on far longer than any listing has room for, \
             so that it has to be cut short somewhere"
        ),
        numbered(4, Shape::Portrait),
    );
    seed.book(
        &format!(
            "{BOOKS}/とても長い名前の本：一覧の幅に収まりきらず途中で省略されることを確かめるための題名"
        ),
        numbered(4, Shape::Portrait),
    );
    seed.book(
        &format!(
            "{BOOKS}/A folder whose name runs on far longer than any listing has room for, \
             to see where it is cut/The book inside it"
        ),
        numbered(4, Shape::Portrait),
    );
    seed.book(
        &format!("{BOOKS}/とても長い名前のフォルダ：一覧の幅に収まりきらず途中で省略されることを確かめるためのフォルダ/中の本"),
        numbered(4, Shape::Portrait),
    );

    // A series: a folder of volumes, each a book.
    for volume in 1..=12 {
        seed.book(
            &format!("{BOOKS}/A series in twelve volumes/Vol. {volume}"),
            numbered(6, Shape::Portrait),
        );
    }

    // Hierarchy and volume.
    seed.book(
        &format!("{BOOKS}/Nested/Two/Three/Four/Five/A book six folders down"),
        numbered(4, Shape::Portrait),
    );
    seed.shelf(&format!("{BOOKS}/A shelf of 300 books"), 300, 2);

    // Photo albums, as the benchmark draws them.
    seed.album(&format!("{ALBUMS}/album-000"), 24);
    seed.album(&format!("{ALBUMS}/album-001"), 24);

    seed.empty(EMPTY);
    seed
}

/// What one image is drawn as.
enum Drawing {
    /// The `index`th page image generated, shown as `position` in its book.
    Page {
        index: usize,
        position: usize,
        size: Size,
    },
    /// The `index`th photo generated.
    Photo { index: usize, size: Size },
}

/// Writes the seed under `out` at `sizes`, and answers what it wrote.
pub fn generate(out: &Path, sizes: Sizes) -> Result<Seed> {
    let seed = layout();

    let mut jobs: Vec<(PathBuf, Drawing)> = Vec::new();
    for book in &seed.books {
        for (position, page) in book.pages.iter().enumerate() {
            let size = match page.shape {
                Shape::Portrait => sizes.portrait,
                Shape::Landscape => sizes.landscape,
                Shape::Spread => sizes.spread,
            };
            let index = jobs.len();
            jobs.push((
                out.join(&book.folder).join(&page.name),
                Drawing::Page {
                    index,
                    position: position + 1,
                    size,
                },
            ));
        }
    }
    let mut photo_index = 0;
    for album in &seed.albums {
        for _ in 0..album.photos {
            jobs.push((
                out.join(&album.folder)
                    .join(format!("img-{photo_index:05}.jpg")),
                Drawing::Photo {
                    index: photo_index,
                    size: sizes.photo,
                },
            ));
            photo_index += 1;
        }
    }

    let folders = jobs
        .iter()
        .map(|(path, _)| {
            path.parent()
                .expect("every image is in a folder")
                .to_path_buf()
        })
        .chain(seed.empty.iter().map(|folder| out.join(folder)));
    for folder in folders {
        std::fs::create_dir_all(&folder)
            .with_context(|| format!("cannot create {}", folder.display()))?;
    }

    jobs.par_iter().try_for_each(|(path, drawing)| {
        let image = match *drawing {
            Drawing::Page {
                index,
                position,
                size,
            } => {
                let mut image = page(index, size);
                crate::digits::stamp(&mut image, position);
                image
            }
            Drawing::Photo { index, size } => photo(index, size),
        };
        save_jpeg(path, image)
    })?;

    Ok(seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny, so that a debug build draws a thousand of them quickly; the
    /// layout does not depend on the size.
    const TINY: Sizes = Sizes {
        portrait: Size::new(12, 18),
        landscape: Size::new(18, 12),
        spread: Size::new(24, 18),
        photo: Size::new(16, 12),
    };

    /// The names of what is directly in `folder`, sorted.
    fn listed(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .unwrap_or_else(|error| panic!("cannot list {}: {error}", folder.display()))
            .map(|entry| {
                entry
                    .expect("an entry of the listing")
                    .file_name()
                    .into_string()
                    .expect("every generated name is UTF-8")
            })
            .collect();
        names.sort();
        names
    }

    fn pages(out: &Path, book: &str) -> Vec<String> {
        listed(&out.join(book))
    }

    fn dimensions(path: &Path) -> (u32, u32) {
        let image = image::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        (image.width(), image.height())
    }

    #[test]
    fn writes_the_seed_in_the_shapes_it_promises() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let out = dir.path();
        let seed = generate(out, TINY).expect("the seed is written");

        // The top-level parts, one mapped prefix each.
        assert_eq!(listed(out), vec![ALBUMS, BOOKS, EMPTY]);
        assert!(listed(&out.join(EMPTY)).is_empty());

        // Books of different shapes.
        assert_eq!(pages(out, "Books/A single page"), vec!["001.jpg"]);
        assert_eq!(pages(out, "Books/A short book").len(), 10);
        let long = pages(out, "Books/A long book");
        assert_eq!(long.len(), 300);
        assert_eq!(long.first().map(String::as_str), Some("001.jpg"));
        assert_eq!(long.last().map(String::as_str), Some("300.jpg"));
        assert_eq!(
            dimensions(&out.join("Books/A short book/001.jpg")),
            (12, 18),
            "portrait"
        );
        assert_eq!(
            dimensions(&out.join("Books/Landscape pages/001.jpg")),
            (18, 12),
            "landscape"
        );
        let mixed = out.join("Books/Single pages and spreads");
        assert_eq!(dimensions(&mixed.join("001.jpg")), (12, 18));
        assert_eq!(dimensions(&mixed.join("002-003.jpg")), (24, 18));
        assert_eq!(dimensions(&mixed.join("014.jpg")), (12, 18));

        let unpadded = pages(out, "Books/Pages out of sequence");
        for name in ["p1.jpg", "p2.jpg", "p10.jpg"] {
            assert!(unpadded.contains(&name.to_owned()), "{name}: {unpadded:?}");
        }
        let matter = pages(out, "Books/A cover and back matter");
        for name in ["cover.jpg", "contents.jpg", "001.jpg", "afterword.jpg"] {
            assert!(matter.contains(&name.to_owned()), "{name}: {matter:?}");
        }

        // Japanese names, for books, folders and pages.
        assert!(pages(out, "Books/吾輩は猫である").contains(&"表紙.jpg".to_owned()));
        assert_eq!(pages(out, "Books/日本の作家/夏目漱石/こころ").len(), 8);

        // Names long enough to be cut short, for books and for folders, in
        // either script.
        let books = listed(&out.join(BOOKS));
        let long_names: Vec<&String> = books
            .iter()
            .filter(|name| name.chars().count() >= 40)
            .collect();
        assert!(
            long_names.iter().any(|name| name.is_ascii())
                && long_names.iter().any(|name| !name.is_ascii()),
            "long names in both scripts: {long_names:?}"
        );
        let long_folders: Vec<&&String> = long_names
            .iter()
            .filter(|name| name.starts_with("A folder") || name.contains("フォルダ"))
            .collect();
        assert_eq!(long_folders.len(), 2, "{long_names:?}");
        for folder in long_folders {
            let inside = listed(&out.join(BOOKS).join(folder));
            assert_eq!(inside.len(), 1, "{folder} holds one book: {inside:?}");
            assert!(
                !pages(out, &format!("{BOOKS}/{folder}/{}", inside[0])).is_empty(),
                "{folder} holds a book"
            );
        }

        // A series of twelve volumes, each a book.
        let volumes = listed(&out.join("Books/A series in twelve volumes"));
        assert_eq!(volumes.len(), 12);
        assert!(volumes.contains(&"Vol. 1".to_owned()));
        assert!(volumes.contains(&"Vol. 12".to_owned()));

        // Nesting several levels deep.
        assert_eq!(
            pages(
                out,
                "Books/Nested/Two/Three/Four/Five/A book six folders down"
            )
            .len(),
            4
        );

        // A shelf of a few hundred small books.
        let shelf = listed(&out.join("Books/A shelf of 300 books"));
        assert_eq!(shelf.len(), 300);
        assert_eq!(pages(out, "Books/A shelf of 300 books/Book 300").len(), 2);

        // Two small photo albums.
        assert_eq!(listed(&out.join(ALBUMS)), vec!["album-000", "album-001"]);
        assert_eq!(pages(out, "Albums/album-001").len(), 24);

        // What is frozen: every book, the shelf as one.
        assert!(seed
            .freeze_units
            .contains(&"Books/A shelf of 300 books".to_owned()));
        assert!(!seed
            .freeze_units
            .iter()
            .any(|unit| unit.starts_with("Books/A shelf of 300 books/")));
        for book in &seed.books {
            assert!(
                seed.freeze_units.iter().any(
                    |unit| book.folder == *unit || book.folder.starts_with(&format!("{unit}/"))
                ),
                "{} is frozen",
                book.folder
            );
        }
    }
}
