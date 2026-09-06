//! The spool capability's own contract, run against the in-memory fake.
//!
//! The fake is what every flow case in this crate spools through, so what it
//! promises has to be what a device's disk promises: a case that arranges an
//! interrupted spool over a fake that quietly tolerated a missing directory, or
//! that refused a second removal, would be arranging a state no device produces.
//! The local filesystem gateway runs the same suite against a real directory.
//!
//! Nothing here is on a filesystem, so the directory the fixture hands over is
//! any path at all — and it is one the fixture never prepares, because
//! preparing it is what one of the cases is about.

use std::path::Path;

use coffret_usecase::spool_conformance::SpoolUnderTest;
use coffret_usecase::InMemoryFs;

/// An empty in-memory filesystem and a directory to spool into.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<SpoolUnderTest> {
    Some(SpoolUnderTest::new(
        Box::new(InMemoryFs::new()),
        Path::new("/spool"),
    ))
}

coffret_usecase::spool_conformance!(fixture().await);
