//! The folder sync's own contract, run against an in-memory store that mints
//! identifiers.
//!
//! The suite's own run uses a store that keys objects by name. Google Drive is
//! the other shape: every reservation mints a fresh identifier, and a commit
//! slot is the identifier its head carries (spec: CP-2). Settling a batch whose
//! commit was attempted reads the slots the batch recorded back through that
//! identifier (spec: OC-3), so the cases about it run here too, over a double
//! of that shape, as part of an ordinary `cargo test`.

use coffret_usecase::sync_conformance::SyncUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryMintingStore};

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects.
const PAGE_SIZE: usize = 3;

/// An empty Library and an empty catalog for one case.
async fn fixture() -> Option<SyncUnderTest> {
    Some(SyncUnderTest::new(
        Box::new(InMemoryMintingStore::new(PAGE_SIZE)),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::sync_conformance!(fixture().await);
