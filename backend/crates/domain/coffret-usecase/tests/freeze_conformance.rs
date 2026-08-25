//! The freeze's own contract, run against the in-memory store and catalog.
//!
//! The suite exists so that packing a folder means the same thing over every
//! backend, and running it here first is what makes a failure elsewhere
//! informative: a case that fails against a real provider and passes here is
//! that provider's disagreement with the port, not the flow's with itself.
//!
//! The folders and the spool directory are real, because a freeze's first step
//! is a walk of a filesystem and there is nothing to fake it with. They are
//! temporary directories this target owns, so an ordinary `cargo test` needs no
//! container and no account.

use coffret_usecase::freeze_conformance::FreezeUnderTest;
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
async fn fixture() -> Option<FreezeUnderTest> {
    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let source = directory.path().join("source");
    let target = directory.path().join("target");
    let spool = directory.path().join("spool");
    for folder in [&source, &target, &spool] {
        std::fs::create_dir_all(folder).expect("making a case's directory must succeed");
    }

    Some(
        FreezeUnderTest::new(
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

coffret_usecase::freeze_conformance!(fixture().await);
