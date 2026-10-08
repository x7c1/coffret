//! The deletion's own contract, run against the in-memory store and catalog.
//!
//! Running it here first is what makes a failure elsewhere informative: a case
//! that fails against a real provider and passes here is that provider's
//! disagreement with the port, not the flow's with itself. The device's folder
//! and spool are in the fixture's own in-memory disk, so an ordinary
//! `cargo test` needs no container, no account, and no directory.

use coffret_usecase::delete_conformance::DeleteUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryStore};

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects, which is where a walk that read one page and stopped
/// would show itself.
const PAGE_SIZE: usize = 3;

/// An empty Library and two empty catalogs, for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<DeleteUnderTest> {
    Some(DeleteUnderTest::new(
        Box::new(InMemoryStore::new(PAGE_SIZE)),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::delete_conformance!(fixture().await);
