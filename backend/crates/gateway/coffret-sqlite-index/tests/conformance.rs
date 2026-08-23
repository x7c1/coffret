//! The Index port's contract, run against a real SQLite file.
//!
//! The same suite the in-memory catalog runs, so that the two cannot quietly
//! mean different things by the port: a device that replays records into a file
//! and one that replays them into memory have to land on the same catalog, or a
//! Snapshot written from either is not the Library's state.
//!
//! It needs nothing but a temporary directory, so it is part of an ordinary
//! `cargo test`.

use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::index_conformance::IndexUnderTest;

/// Two catalogs in two files, in a directory that goes away with the case.
///
/// Two files rather than two connections to one: the suite's comparisons are
/// between independent devices' catalogs, and sharing a file would let one
/// case's writes answer the other's reads.
async fn fixture() -> Option<IndexUnderTest> {
    let directory = tempfile::tempdir().expect("a temporary directory must be creatable");
    let index = SqliteIndex::open(directory.path().join("index.sqlite"))
        .expect("opening a fresh Index file must succeed");
    let other = SqliteIndex::open(directory.path().join("other.sqlite"))
        .expect("opening a second fresh Index file must succeed");

    Some(IndexUnderTest::new(Box::new(index), Box::new(other)).holding(Box::new(directory)))
}

coffret_usecase::index_conformance!(fixture().await);
