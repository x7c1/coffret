//! The fetch's own contract, run against the in-memory store and catalog.
//!
//! The suite exists so that a fetch means the same thing over every backend, and
//! running it here first is what makes a failure elsewhere informative: a case
//! that fails against a real provider and passes here is that provider's
//! disagreement with the port, not the flow's with itself.
//!
//! Two catalogs, because every case syncs from one device and fetches into
//! another — what a fetch is worth is what a device that did not make the
//! Library gets out of it. Neither device's folder is on a filesystem: both are
//! in the fixture's own in-memory disk, because everything either of them does
//! to a local file goes through a capability — the source device scans and
//! spools, and the target device places.

use coffret_usecase::fetch_conformance::FetchUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryStore};

/// Small enough that a case reaches a second listing page while writing only a
/// handful of objects, which is where a walk that read one page and stopped
/// would show itself.
const PAGE_SIZE: usize = 3;

/// An empty Library and two empty catalogs, for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<FetchUnderTest> {
    Some(FetchUnderTest::new(
        Box::new(InMemoryStore::new(PAGE_SIZE)),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::fetch_conformance!(fixture().await);

// The sweep at open says how many parcel files it discarded, in one line, so a
// person reading the log can tell a device that had ciphertext nothing named
// from one that had none (spec: PK-21). The line carries the count and neither
// a Container nor a path on this device (spec: EL-1).
#[tokio::test]
async fn the_sweep_at_open_logs_how_many_parcel_files_it_discarded() {
    let logs = coffret_logging::testing::CapturedLogs::capture();
    let fixture = fixture()
        .await
        .expect("the in-memory fixture is always there");
    coffret_usecase::fetch_conformance::a_parcel_file_no_row_names_is_discarded_at_open(&fixture)
        .await;

    let swept: Vec<_> = logs
        .at(tracing::Level::INFO)
        .into_iter()
        .filter(|event| event.message().starts_with("discarded the parcel files"))
        .collect();
    assert_eq!(
        swept.len(),
        1,
        "one line for the one sweep that discarded anything"
    );
    assert_eq!(swept[0].field("operation"), "sweep_parcels");
    assert_eq!(swept[0].number("parcels"), 1);
    logs.assert_free_of(&[".parcel", "books"]);
}
