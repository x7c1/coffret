//! The commit conformance suite, run against a real S3 implementation.
//!
//! The in-memory run of this suite proves the flow is self-consistent. What it
//! cannot prove is the one thing the whole commit protocol rests on: that a
//! conditional create against a real object store is exclusive, that a refused
//! one is reported as a lost race and not as a fault, and that a writer refused
//! by it can read back what took the slot. So the same cases run here, against a
//! server that actually evaluates `If-None-Match: *`.
//!
//! The catalogs stay in memory. The Index is a device-local cache with a
//! contract of its own, held by `index_conformance`, and pairing it with a real
//! Storage here would only make a failure harder to place.
//!
//! `make s3-store-it` supplies the environment; without it the cases report
//! themselves skipped.

use coffret_usecase::commit_conformance::CommitUnderTest;
use coffret_usecase::InMemoryIndex;

mod minio;

/// Hands the suite an empty Library and two empty catalogs, or `None` when no
/// endpoint is configured.
async fn fixture() -> Option<CommitUnderTest> {
    let (store, _page_size) = minio::store("commit").await?;
    Some(CommitUnderTest::new(
        Box::new(store),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::commit_conformance!(fixture().await);
