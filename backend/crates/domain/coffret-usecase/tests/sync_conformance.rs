//! The folder sync's own contract, run against the in-memory store and catalog.
//!
//! The suite exists so that a sync means the same thing over every backend, and
//! running it here first is what makes a failure elsewhere informative: a case
//! that fails against a real provider and passes here is that provider's
//! disagreement with the port, not the flow's with itself.
//!
//! The folder and the spool directory are real, because a sync's first step is
//! a walk of a filesystem and there is nothing to fake it with. They are
//! temporary directories this target owns, so an ordinary `cargo test` needs no
//! container and no account.

use coffret_usecase::sync_conformance::SyncUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryStore};
use tempfile::TempDir;

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects, which is where a walk that read one page and stopped
/// would show itself.
const PAGE_SIZE: usize = 3;

/// An empty Library, an empty catalog, and two empty directories for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<SyncUnderTest> {
    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let folder = directory.path().join("folder");
    let spool = directory.path().join("spool");
    std::fs::create_dir_all(&folder).expect("making the mapped folder must succeed");
    std::fs::create_dir_all(&spool).expect("making the spool directory must succeed");

    Some(
        SyncUnderTest::new(
            Box::new(InMemoryStore::new(PAGE_SIZE)),
            Box::new(InMemoryIndex::new()),
            folder,
            spool,
        )
        // Dropping it removes both directories, so a case that panics leaves
        // nothing behind either.
        .holding(Box::new(directory)),
    )
}

coffret_usecase::sync_conformance!(fixture().await);
