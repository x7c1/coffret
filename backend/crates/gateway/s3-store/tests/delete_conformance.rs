//! The deletion conformance suite, run against a real S3 implementation.
//!
//! The in-memory run proves the flow is self-consistent. What it cannot prove is
//! that a Pack read back out of a bucket to be rebuilt is the Pack that went in,
//! that the replacement written from it arrives whole, and that what a deletion
//! removes really leaves the listing — so the same cases run here, against a
//! server that stores the bytes and answers for them.
//!
//! The catalogs, the folder and the spool stay in memory, for the reason the
//! freeze target gives: what this target is for is Storage.
//!
//! `make s3-store-it` supplies the environment; without it the cases report
//! themselves skipped.

use coffret_usecase::delete_conformance::DeleteUnderTest;
use coffret_usecase::InMemoryIndex;

mod minio;

/// Hands the suite an empty Library in a bucket and two empty catalogs, or
/// `None` when no endpoint is configured.
async fn fixture() -> Option<DeleteUnderTest> {
    let (store, _page_size) = minio::store("delete").await?;

    Some(DeleteUnderTest::new(
        Box::new(store),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::delete_conformance!(fixture().await);
