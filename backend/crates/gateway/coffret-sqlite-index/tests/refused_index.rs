//! Reading the mappings out of a file `SqliteIndex::open` refuses.
//!
//! The two columns that carry a mapping — `prefix` and `local_root` — are the
//! one part of the device-local group every layout has kept, all the way back
//! to the first. That is what lets [`RefusedIndex`] answer where a
//! `prepare`-based open cannot: it reads only those two columns, runs no
//! layout check, and leaves the file exactly as it found it — which is the
//! one promise a refusal makes.

use std::path::PathBuf;

use coffret_sqlite_index::{RefusedIndex, SqliteIndex};
use coffret_usecase::device_state::Mapping;
use coffret_usecase::IndexError;

mod support;

use support::{entry_path, rows_in, stamp_of, Scratch};

/// The layout this build writes, and the one its device-local group last
/// changed at.
///
/// Written out rather than read from the adapter, which keeps them to itself
/// — the sibling suite beside this one does the same.
const SCHEMA_VERSION: i64 = 5;
const DEVICE_SCHEMA_VERSION: i64 = 4;

/// A `mappings` table shaped `columns`, holding two rows, stamped `version`
/// afterwards.
///
/// Built with `rusqlite` directly rather than through the adapter: `SqliteIndex`
/// only ever writes its own current layout, so a file at an older one — or in
/// a shape no layout of this build ever wrote — has to be built behind its
/// back, the way every other case in this crate that needs an old file does.
fn a_file_with_mappings(scratch: &Scratch, columns: &str, version: i64) {
    let connection = rusqlite::Connection::open(scratch.file()).expect("the Index file must open");
    connection
        .execute_batch(&format!("CREATE TABLE mappings ({columns})"))
        .expect("creating the mappings table must succeed");
    connection
        .execute(
            "INSERT INTO mappings (prefix, local_root) VALUES (?1, ?2)",
            rusqlite::params![None::<&str>, "/somewhere"],
        )
        .expect("inserting the root mapping must succeed");
    connection
        .execute(
            "INSERT INTO mappings (prefix, local_root) VALUES (?1, ?2)",
            rusqlite::params![Some("albums"), "/somewhere/albums"],
        )
        .expect("inserting a mapping must succeed");
    connection
        .pragma_update(None, "user_version", version)
        .expect("stamping a version must succeed");
}

/// The shape the device-local group's `mappings` table has had since layout 1,
/// and still has: `prefix` and `local_root`, and nothing this build asks for
/// by name beyond them.
const CURRENT_SHAPE: &str = "prefix TEXT, local_root TEXT NOT NULL, root_identity TEXT";

/// A file `SqliteIndex::open` would refuse still gives its mappings up, root
/// first and with no `root_identity`: a mapping read this way is about to be
/// recorded afresh, so the next scan is what stamps it, not this read.
#[tokio::test]
async fn mappings_are_read_from_a_refused_file() {
    let scratch = Scratch::new();
    a_file_with_mappings(&scratch, CURRENT_SHAPE, DEVICE_SCHEMA_VERSION - 1);

    let read = RefusedIndex::open(scratch.file())
        .expect("a refused file must still open for reading its mappings")
        .mappings()
        .expect("the two columns every layout keeps must read back");

    assert_eq!(
        read,
        vec![
            Mapping {
                prefix: None,
                local_root: PathBuf::from("/somewhere"),
                root_identity: None,
            },
            Mapping {
                prefix: Some(entry_path("albums")),
                local_root: PathBuf::from("/somewhere/albums"),
                root_identity: None,
            },
        ]
    );
}

/// Reading a refused file this way is not a write: the stamp and every row
/// count stay exactly as they were, and the file the port still refuses is
/// still that same file.
#[tokio::test]
async fn reading_a_refused_file_leaves_it_as_it_was() {
    let scratch = Scratch::new();
    a_file_with_mappings(&scratch, CURRENT_SHAPE, DEVICE_SCHEMA_VERSION - 1);

    RefusedIndex::open(scratch.file())
        .expect("a refused file must still open for reading its mappings")
        .mappings()
        .expect("the two columns every layout keeps must read back");

    assert_eq!(
        stamp_of(&scratch.file()),
        DEVICE_SCHEMA_VERSION - 1,
        "reading the mappings issues no write, so the stamp is exactly as it was"
    );
    assert_eq!(rows_in(&scratch.file(), "mappings"), 2, "and no row moved");

    let reopened = SqliteIndex::open(scratch.file());
    assert!(
        matches!(
            reopened.as_ref().err(),
            Some(IndexError::UnsupportedSchema { found, supported })
                if *found == DEVICE_SCHEMA_VERSION - 1 && *supported == SCHEMA_VERSION
        ),
        "the file is still the layout the port refuses, got {:?}",
        reopened.err()
    );
}

/// Only `prefix` and `local_root` are ever asked for by name, so a `mappings`
/// table in the layout-1 shape — with no `root_identity` at all — and one
/// carrying a column no layout of this build has ever heard of both read back
/// the same two mappings.
#[tokio::test]
async fn a_refused_file_needs_only_the_two_columns_every_layout_keeps() {
    for columns in [
        "prefix TEXT, local_root TEXT NOT NULL",
        "prefix TEXT, local_root TEXT NOT NULL, root_identity TEXT, guessed_kind TEXT",
    ] {
        let scratch = Scratch::new();
        a_file_with_mappings(&scratch, columns, DEVICE_SCHEMA_VERSION - 1);

        let read = RefusedIndex::open(scratch.file())
            .expect("a refused file must still open for reading its mappings")
            .mappings()
            .expect("only prefix and local_root are asked for, and both are there");
        assert_eq!(
            read.len(),
            2,
            "the shape {columns:?} must still yield both mappings"
        );
    }
}
