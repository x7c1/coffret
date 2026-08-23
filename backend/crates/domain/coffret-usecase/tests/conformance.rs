//! The port's own contract, run against the in-memory store.
//!
//! The suite exists so that two adapters cannot quietly mean different things
//! by the port, and the store the crate's own cases drive is held to it for the
//! same reason: a commit-protocol case proves nothing about the protocol if the
//! store underneath it is not a faithful `ObjectStore`.
//!
//! It needs no container and no account, so unlike the gateways' runs it is
//! part of an ordinary `cargo test`.

use coffret_usecase::conformance::StoreUnderTest;
use coffret_usecase::InMemoryStore;

/// Small enough that the pagination case writes only a handful of objects.
const PAGE_SIZE: usize = 3;

/// A fresh empty store for one case.
///
/// Async because the macro awaits it, as a gateway's fixture must be.
async fn fixture() -> Option<StoreUnderTest> {
    Some(StoreUnderTest::new(
        Box::new(InMemoryStore::new(PAGE_SIZE)),
        PAGE_SIZE,
    ))
}

coffret_usecase::object_store_conformance!(fixture().await);
