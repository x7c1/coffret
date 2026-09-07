//! The folder sync's own contract, run against the in-memory store and catalog.
//!
//! The suite exists so that a sync means the same thing over every backend, and
//! running it here first is what makes a failure elsewhere informative: a case
//! that fails against a real provider and passes here is that provider's
//! disagreement with the port, not the flow's with itself.
//!
//! Neither the mapped folder nor the spool is a real directory: both are the
//! fixture's own in-memory filesystem, which is what lets the cases about a root
//! that is not there, a folder that will not list, and a spool that will not
//! flush be about a disk that refuses (spec: EP-12, OC-2). So an ordinary
//! `cargo test` needs no container, no account, and no temporary directory.

use coffret_usecase::sync_conformance::SyncUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryStore};

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects, which is where a walk that read one page and stopped
/// would show itself.
const PAGE_SIZE: usize = 3;

/// An empty Library and an empty catalog for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<SyncUnderTest> {
    Some(SyncUnderTest::new(
        Box::new(InMemoryStore::new(PAGE_SIZE)),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::sync_conformance!(fixture().await);
