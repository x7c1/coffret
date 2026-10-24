//! The port's own contract, run against the in-memory store that mints
//! identifiers.
//!
//! `InMemoryMintingStore` stands in for Google Drive wherever this crate's
//! cases need a store of that shape — the commit and sync suites run over it in
//! every `cargo test` — and a double proves something about the protocol above
//! it only as far as it is a faithful `ObjectStore` underneath. The name-keyed
//! store is held to the port by `conformance`; this holds the minting one to
//! the same contract, so a drift between the two doubles is a failure here
//! rather than a case elsewhere that passes for the wrong reason.
//!
//! Every case runs. None of them assumes names are unique or that two
//! reservations of one name meet: the conditional-create cases each consume the
//! one slot they reserved, as a writer consumes the slot its head carries, and
//! the listing cases store each object under a name of its own.
//!
//! It needs no account, so it is part of an ordinary `cargo test`.

use coffret_usecase::conformance::StoreUnderTest;
use coffret_usecase::InMemoryMintingStore;

/// Small enough that the pagination case writes only a handful of objects.
const PAGE_SIZE: usize = 3;

/// A fresh empty store for one case.
///
/// Async because the macro awaits it, as a gateway's fixture must be.
async fn fixture() -> Option<StoreUnderTest> {
    Some(StoreUnderTest::new(
        Box::new(InMemoryMintingStore::new(PAGE_SIZE)),
        PAGE_SIZE,
    ))
}

coffret_usecase::object_store_conformance!(fixture().await);
