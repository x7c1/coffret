//! The `ObjectStore` conformance suite, run against a real Google Drive folder.
//!
//! The same suite the S3 gateway runs in CI, pointed at Drive. It is not part of
//! any automated run and never will be: it needs a Google account, a grant a
//! person clicked through, and calls against a live API. It exists so that the
//! two adapters are held to one contract rather than to whatever each provider
//! happened to make easy, and it is run by hand when the Drive adapter changes.
//!
//! Authorize once with the `authorize` example, then set:
//!
//! ```text
//! COFFRET_DRIVE_FOLDER_ID      the folder to work in; its presence turns the suite on
//! COFFRET_DRIVE_CLIENT_ID      the OAuth client to authorize as
//! COFFRET_DRIVE_CLIENT_SECRET  optional, for a client registered with one
//! COFFRET_DRIVE_TOKEN_CACHE    where the grant was cached
//! COFFRET_MASTER_KEY           the Master Key that cache was sealed under
//! ```
//!
//! The cache is encrypted, so `COFFRET_MASTER_KEY` has to be the same value the
//! `authorize` example ran with; under any other one the cache does not open and
//! the suite fails before its first call.
//!
//! `COFFRET_DRIVE_FOLDER_ID` may be any folder — one made in the Drive web
//! interface included. A `drive.file` grant reaches only what this application
//! created, but that restricts what may be *read*, not where something new may
//! be put: naming a folder as the parent of a file being created is allowed,
//! and each case only creates a subfolder of its own and stays inside it, so it
//! never asks to read the folder it was given. `root` is the exception, and
//! `MY_DRIVE` says why.
//!
//! Each case works in a subfolder of its own, so the cases neither see each
//! other's objects nor need the configured folder to be empty. The subfolders
//! are left behind: they are the record of a run, and deleting them from a case
//! that failed would delete the evidence.
//!
//! So is the log. A configured run writes every call it makes to a file under
//! `$XDG_STATE_HOME/coffret/logs` — `$HOME/.local/state/coffret/logs` where
//! that is unset — and prints the name of it as it starts. That file is what
//! answers "what does Drive actually send when this happens?" afterwards, which
//! is the question this suite exists to have an answer to — asked of the file
//! with `jq`, since it is JSONL and every answer Drive gave is a field rather
//! than a phrase in a line. `COFFRET_LOG_DIR` moves it and
//! `COFFRET_LOG_MAX_BYTES` bounds how much is kept.

mod support;

use coffret_usecase::conformance::StoreUnderTest;

/// How many objects one listing page holds during the suite.
///
/// Small, so the pagination case reaches a second page by writing a handful of
/// objects instead of a thousand.
const PAGE_SIZE: i32 = 2;

/// Hands the suite an empty store, or `None` when Drive is not configured.
async fn fixture() -> Option<StoreUnderTest> {
    let drive = support::drive(|settings| settings.with_page_size(PAGE_SIZE)).await?;

    Some(StoreUnderTest::new(Box::new(drive), PAGE_SIZE as usize))
}

coffret_usecase::object_store_conformance!(fixture().await);
