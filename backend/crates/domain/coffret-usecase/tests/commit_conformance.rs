//! The commit flow's own contract, run against the in-memory store and catalog.
//!
//! The suite exists so that the commit protocol means the same thing over every
//! backend, and running it here first is what makes a failure elsewhere
//! informative: a case that fails against a real provider and passes here is
//! that provider's disagreement with the port, not the flow's with itself.
//!
//! It needs no container and no account, so unlike a gateway's run it is part of
//! an ordinary `cargo test`.

use coffret_usecase::commit_conformance::CommitUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryStore};

/// Small enough that the cases reach a second listing page while writing only a
/// handful of objects, which is where a catch-up that read one page and stopped
/// would show itself.
const PAGE_SIZE: usize = 3;

/// An empty Library and two empty catalogs for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<CommitUnderTest> {
    Some(CommitUnderTest::new(
        Box::new(InMemoryStore::new(PAGE_SIZE)),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::commit_conformance!(fixture().await);
