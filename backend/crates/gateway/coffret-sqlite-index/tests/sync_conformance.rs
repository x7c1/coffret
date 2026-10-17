//! The folder sync's own contract, run over a real SQLite catalog.
//!
//! The suite the use-case crate runs over its in-memory catalog, here over
//! this adapter's: what a sync makes of the records it keeps — which Entry each
//! materialized file is a copy of, which wait to follow a rename, which are
//! relocated, forgotten or departed — is the catalog's to keep, so the flow is
//! held to the same answers over both (spec: EP-10). The Library and the
//! device's disk stay in memory: what differs here is the catalog alone.
//!
//! It needs nothing but a temporary directory, so it is part of an ordinary
//! `cargo test`.

use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::sync_conformance::SyncUnderTest;
use coffret_usecase::InMemoryStore;

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects.
const PAGE_SIZE: usize = 3;

/// An empty Library, and an empty catalog in a file that goes away with the
/// case.
async fn fixture() -> Option<SyncUnderTest> {
    let directory = tempfile::tempdir().expect("a temporary directory must be creatable");
    let index = SqliteIndex::open(directory.path().join("index.sqlite"))
        .expect("opening a fresh Index file must succeed");
    Some(
        SyncUnderTest::new(Box::new(InMemoryStore::new(PAGE_SIZE)), Box::new(index))
            .holding(Box::new(directory)),
    )
}

coffret_usecase::sync_conformance!(fixture().await);
