//! The fetch's own contract, run against the in-memory store and catalog.
//!
//! The suite exists so that a fetch means the same thing over every backend, and
//! running it here first is what makes a failure elsewhere informative: a case
//! that fails against a real provider and passes here is that provider's
//! disagreement with the port, not the flow's with itself.
//!
//! Two catalogs and two folders, because every case syncs from one device and
//! fetches into another — what a fetch is worth is what a device that did not
//! make the Library gets out of it. The folders are real, because a fetch ends at
//! a filesystem and there is nothing to fake it with. They are temporary
//! directories this target owns, so an ordinary `cargo test` needs no container
//! and no account.

use coffret_usecase::fetch_conformance::FetchUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryStore};
use tempfile::TempDir;

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects, which is where a walk that read one page and stopped
/// would show itself.
const PAGE_SIZE: usize = 3;

/// An empty Library, two empty catalogs, and three empty directories for one
/// case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<FetchUnderTest> {
    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let source = directory.path().join("source");
    let target = directory.path().join("target");
    let spool = directory.path().join("spool");
    for folder in [&source, &target, &spool] {
        std::fs::create_dir_all(folder).expect("making a case's directory must succeed");
    }

    Some(
        FetchUnderTest::new(
            Box::new(InMemoryStore::new(PAGE_SIZE)),
            Box::new(InMemoryIndex::new()),
            source,
            Box::new(InMemoryIndex::new()),
            target,
            spool,
        )
        // Dropping it removes all three directories, so a case that panics
        // leaves nothing behind either.
        .holding(Box::new(directory)),
    )
}

coffret_usecase::fetch_conformance!(fixture().await);
