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
//! it with a real Storage here would only make a failure harder to place. The
//! folders and the spool directory are temporary directories of this target's,
//! because a fetch ends at a real filesystem whichever provider the Library is
//! on.
//!
//! `make s3-store-it` supplies the environment; without it the cases report
//! themselves skipped.

use coffret_usecase::fetch_conformance::FetchUnderTest;
use coffret_usecase::InMemoryIndex;
use tempfile::TempDir;

mod minio;

/// Hands the suite an empty Library, two empty catalogs, and three empty
/// directories, or `None` when no endpoint is configured.
async fn fixture() -> Option<FetchUnderTest> {
    let (store, _page_size) = minio::store("fetch").await?;

    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let source = directory.path().join("source");
    let target = directory.path().join("target");
    let spool = directory.path().join("spool");
    for folder in [&source, &target, &spool] {
        std::fs::create_dir_all(folder).expect("making a case's directory must succeed");
    }

    Some(
        FetchUnderTest::new(
            Box::new(store),
            Box::new(InMemoryIndex::new()),
            source,
            Box::new(InMemoryIndex::new()),
            target,
            spool,
        )
        // Dropping it removes all three directories, so a case that panics
        // leaves nothing behind either.
        .holding(Box::new(directory)),
    )
}

coffret_usecase::fetch_conformance!(fixture().await);
