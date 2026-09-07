//! The freeze conformance suite, run against a real S3 implementation.
//!
//! The in-memory run of this suite proves the flow is self-consistent. What it
//! cannot prove is what a Pack is for: that a Container holding several files
//! really arrives in a bucket whole, that the digest a server reports for it is
//! the digest of what left this device, and that a device which never saw the
//! Library can pull the folder back out of those Packs and find the files that
//! were on another device's disk. So the same cases run here, against a server
//! that stores the bytes and answers for them.
//!
//! The catalogs stay in memory, for the reason the commit, sync, and fetch
//! targets give: the Index has a contract of its own, held by
//! `index_conformance`, and pairing it with a real Storage here would only make
//! a failure harder to place. So do both devices' folders and the spool between
//! them, held to their contracts by `mapped_roots_conformance`,
//! `spool_conformance`, and `destinations_conformance` — what this target is for
//! is Storage, and a real filesystem on either side of it would only be a second
//! thing that could fail.
//!
//! `make s3-store-it` supplies the environment; without it the cases report
//! themselves skipped.

use coffret_usecase::freeze_conformance::FreezeUnderTest;
use coffret_usecase::InMemoryIndex;

mod minio;

/// Hands the suite an empty Library in a bucket and two empty catalogs, or
/// `None` when no endpoint is configured.
async fn fixture() -> Option<FreezeUnderTest> {
    let (store, _page_size) = minio::store("freeze").await?;

    Some(FreezeUnderTest::new(
        Box::new(store),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::freeze_conformance!(fixture().await);
