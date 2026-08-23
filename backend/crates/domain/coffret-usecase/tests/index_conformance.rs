//! The Index port's own contract, run against the in-memory catalog.
//!
//! The suite exists so that two implementations cannot quietly mean different
//! things by the port, and the catalog the crate's own cases drive is held to it
//! for the same reason: a case about catching up proves nothing if the catalog
//! underneath it is not a faithful `Index`.
//!
//! It touches no file and needs nothing configured, so unlike an adapter's run
//! it is part of an ordinary `cargo test`.

use coffret_usecase::index_conformance::IndexUnderTest;
use coffret_usecase::InMemoryIndex;

/// Two empty, independent catalogs for one case.
///
/// Async because the macro awaits it, as an adapter's fixture must be.
async fn fixture() -> Option<IndexUnderTest> {
    Some(IndexUnderTest::new(
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::index_conformance!(fixture().await);
