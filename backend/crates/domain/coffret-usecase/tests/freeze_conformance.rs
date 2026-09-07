//! The freeze's own contract, run against the in-memory store and catalog.
//!
//! The suite exists so that packing a folder means the same thing over every
//! backend, and running it here first is what makes a failure elsewhere
//! informative: a case that fails against a real provider and passes here is
//! that provider's disagreement with the port, not the flow's with itself.
//!
//! The folder the freezing device packs is not a real one: it is the fixture's
//! own in-memory filesystem, along with the spool, which is what lets the cases
//! about a root that is not there and an interrupted run be about a disk that
//! refuses (spec: EP-12, OC-2). The folder the *second* device fetches into is
//! real, because a fetch places bytes through the operating system — a temporary
//! directory this target owns, so an ordinary `cargo test` needs no container
//! and no account.

use coffret_usecase::freeze_conformance::FreezeUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryStore};
use tempfile::TempDir;

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects, which is where a walk that read one page and stopped
/// would show itself.
const PAGE_SIZE: usize = 3;

/// An empty Library, two empty catalogs, and the empty directory the second
/// device fetches into, for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<FreezeUnderTest> {
    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let target = directory.path().join("target");
    std::fs::create_dir_all(&target).expect("making a case's directory must succeed");

    Some(
        FreezeUnderTest::new(
            Box::new(InMemoryStore::new(PAGE_SIZE)),
            Box::new(InMemoryIndex::new()),
            Box::new(InMemoryIndex::new()),
            target,
        )
        // Dropping it removes the directory, so a case that panics leaves
        // nothing behind either.
        .holding(Box::new(directory)),
    )
}

coffret_usecase::freeze_conformance!(fixture().await);
