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
//! a failure harder to place. The folder the freezing device packs and the spool
//! it writes are the fixture's own in-memory filesystem, held to their contracts
//! by `mapped_roots_conformance` and `spool_conformance`; the folder the second
//! device fetches into is a temporary directory of this target's, because a
//! fetch places bytes through the operating system whichever provider the
//! Library is on.
//!
//! `make s3-store-it` supplies the environment; without it the cases report
//! themselves skipped.

use coffret_usecase::freeze_conformance::FreezeUnderTest;
use coffret_usecase::InMemoryIndex;
use tempfile::TempDir;

mod minio;

/// Hands the suite an empty Library, two empty catalogs, and the empty directory
/// the second device fetches into, or `None` when no endpoint is configured.
async fn fixture() -> Option<FreezeUnderTest> {
    let (store, _page_size) = minio::store("freeze").await?;

    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let target = directory.path().join("target");
    std::fs::create_dir_all(&target).expect("making a case's directory must succeed");

    Some(
        FreezeUnderTest::new(
            Box::new(store),
            Box::new(InMemoryIndex::new()),
            Box::new(InMemoryIndex::new()),
            target,
        )
        // Dropping it removes the directory, so a case that panics leaves
        // nothing behind either.
        .holding(Box::new(directory)),
    )
}

coffret_usecase::freeze_conformance!(fixture().await);
