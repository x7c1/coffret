//! The commit conformance suite, run against a real Google Drive folder.
//!
//! The commit protocol on Drive rests on two things Drive does: an object can be
//! created under an id another writer minted with `files.generateIds` under the
//! same grant, and a create under an id that names a live file is refused as a
//! lost race rather than a fault. `coffret-usecase` runs this suite over
//! `InMemoryMintingStore` in every `cargo test`, and that double is built on
//! exactly those two assumptions — so it passes whatever Drive actually
//! answers. This run is what holds the assumptions to Drive itself: the same
//! cases, against the API they were made about.
//!
//! Every case runs here, as in the minting run, the races for a Library's first
//! commit among them: every case starts from a Library whose creation wrote
//! `head-0` carrying the slot of its successor, so two writers racing for the
//! first commit consume the one id `head-0` carries.
//!
//! The catalogs stay in memory, as in the S3 run. The Index is a device-local
//! cache with a contract of its own, held by `index_conformance`, and pairing it
//! with a real Storage here would only make a failure harder to place.
//!
//! It takes the same environment as the `ObjectStore` run in `conformance.rs`
//! (see its module docs): each case works in a subfolder of its own under
//! `COFFRET_DRIVE_FOLDER_ID`, trashed when the case ends, and without the
//! variables the cases report themselves skipped. Like that one it is never
//! part of an automated run: it needs an account and a grant a person clicked
//! through. `make drive-store-it` runs it.

mod support;

use coffret_usecase::commit_conformance::CommitUnderTest;
use coffret_usecase::InMemoryIndex;

/// How many objects one listing page holds during the suite.
///
/// Small, so the cases reach a second listing page, as in the S3 and minting
/// runs.
const PAGE_SIZE: i32 = 2;

/// Hands the suite an empty Library and two empty catalogs, or `None` when
/// Drive is not configured.
async fn fixture() -> Option<CommitUnderTest> {
    let drive = support::drive(|settings| settings.with_page_size(PAGE_SIZE)).await?;

    Some(CommitUnderTest::new(
        Box::new(drive),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::commit_conformance!(fixture().await);
