//! The fetch's own contract, run against the in-memory store and catalog.
//!
//! The suite exists so that a fetch means the same thing over every backend, and
//! running it here first is what makes a failure elsewhere informative: a case
//! that fails against a real provider and passes here is that provider's
//! disagreement with the port, not the flow's with itself.
//!
//! Two catalogs, because every case syncs from one device and fetches into
//! another — what a fetch is worth is what a device that did not make the
//! Library gets out of it. One folder, and it is the fetching device's: a fetch
//! ends at a real filesystem, so that one is a temporary directory this target
//! owns. The source device's folder and spool are both the fixture's own
//! in-memory filesystem, because all that device does on the way in is scan and
//! upload.

use coffret_usecase::fetch_conformance::FetchUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryStore};
use tempfile::TempDir;

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects, which is where a walk that read one page and stopped
/// would show itself.
const PAGE_SIZE: usize = 3;

/// An empty Library, two empty catalogs, and the empty directory the fetching
/// device places into, for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<FetchUnderTest> {
    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let target = directory.path().join("target");
    std::fs::create_dir_all(&target).expect("making a case's directory must succeed");

    Some(
        FetchUnderTest::new(
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

coffret_usecase::fetch_conformance!(fixture().await);
