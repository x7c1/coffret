//! The fetch conformance suite, run against a real S3 implementation.
//!
//! The in-memory run of this suite proves the flow is self-consistent. What it
//! cannot prove is the half of the round trip this side is for: that a device
//! which never saw the Library can rebuild its catalog from what is really in a
//! bucket, open the Keyring the checkpoint there names, pull each Container back
//! out, and find the file that was on another device's disk. So the same cases run
//! here, against a server that stores the bytes and answers for them.
//!
//! The catalogs stay in memory, for the reason the commit and sync targets give:
//! the Index has a contract of its own, held by `index_conformance`, and pairing
//! it with a real Storage here would only make a failure harder to place. So do
//! both devices' folders and the spool between them, held to their contracts by
//! `mapped_roots_conformance`, `spool_conformance`, and
//! `destinations_conformance` — what this target is for is Storage, and a real
//! filesystem on either side of it would only be a second thing that could
//! fail.
//!
//! `make s3-store-it` supplies the environment; without it the cases report
//! themselves skipped.

use coffret_usecase::fetch_conformance::FetchUnderTest;
use coffret_usecase::InMemoryIndex;

mod minio;

/// Hands the suite an empty Library in a bucket and two empty catalogs, or
/// `None` when no endpoint is configured.
async fn fixture() -> Option<FetchUnderTest> {
    let (store, _page_size) = minio::store("fetch").await?;

    Some(FetchUnderTest::new(
        Box::new(store),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::fetch_conformance!(fixture().await);
