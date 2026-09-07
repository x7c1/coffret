//! The spool capability's own contract, run against a real directory.
//!
//! The same suite the use-case crate runs against its in-memory fake, so that
//! the fake a flow case is driven over and the filesystem a device actually
//! spools onto are held to one contract. A case that passes there and fails here
//! is this gateway's disagreement with the capability, not the flow's with
//! itself.
//!
//! The directory is a temporary one this target owns, so an ordinary
//! `cargo test` needs no state directory and leaves nothing behind.

use coffret_local_fs::UnixFs;
use coffret_usecase::spool_conformance::SpoolUnderTest;
use tempfile::TempDir;

/// A real filesystem and a directory nobody has made yet, for one case.
///
/// The directory it hands over is one *inside* the temporary one and is
/// deliberately not created: preparing it is `Spool::prepare_dir`'s to do, and
/// a case about a file created under a directory nobody made needs somewhere
/// that was never made.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<SpoolUnderTest> {
    let directory = TempDir::new().expect("making a temporary directory must succeed");
    let spool = directory.path().join("spool");

    Some(
        SpoolUnderTest::new(Box::new(UnixFs::new()), spool)
            // Dropping it removes the directory, so a case that panics leaves
            // nothing behind either.
            .holding(Box::new(directory)),
    )
}

coffret_usecase::spool_conformance!(fixture().await);
