//! The folder sync conformance suite, run against a real S3 implementation.
//!
//! The in-memory run of this suite proves the flow is self-consistent. What it
//! cannot prove is the round trip the whole path exists to make: that the
//! ciphertext a sync sends really arrives, that the digest a server reports for
//! what it stored is the digest of what left this device, and that a Container
//! fetched back out of a bucket opens under the envelope the committed Keyring
//! holds for it and decodes to the file that was on disk. So the same cases run
//! here, against a server that stores the bytes and answers for them.
//!
//! The catalog stays in memory, for the reason the commit target gives: the
//! Index has a contract of its own, held by `index_conformance`, and pairing it
//! with a real Storage here would only make a failure harder to place. The
//! folder and the spool directory are temporary directories of this target's,
//! because a sync's first step is a walk of a real filesystem whichever
//! provider the Library is on.
//!
//! `make s3-store-it` supplies the environment; without it the cases report
//! themselves skipped.

use coffret_usecase::sync_conformance::SyncUnderTest;
use coffret_usecase::InMemoryIndex;
use tempfile::TempDir;

mod minio;

/// Hands the suite an empty Library, an empty catalog, and two empty
/// directories, or `None` when no endpoint is configured.
async fn fixture() -> Option<SyncUnderTest> {
    let (store, _page_size) = minio::store("sync").await?;

    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let folder = directory.path().join("folder");
    let spool = directory.path().join("spool");
    std::fs::create_dir_all(&folder).expect("making the mapped folder must succeed");
    std::fs::create_dir_all(&spool).expect("making the spool directory must succeed");

    Some(
        SyncUnderTest::new(
            Box::new(store),
            Box::new(InMemoryIndex::new()),
            folder,
            spool,
        )
        // Dropping it removes both directories, so a case that panics leaves
        // nothing behind either.
        .holding(Box::new(directory)),
    )
}

coffret_usecase::sync_conformance!(fixture().await);
