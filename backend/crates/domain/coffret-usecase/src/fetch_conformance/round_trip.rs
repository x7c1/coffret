use crate::device_state::LocalEntryState;
use crate::entry_paths::entry_path;
use crate::fetch::fetch_folders;
use crate::fetch_conformance::counting_store::CountingStore;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    entry_at, keys, map, observed, read, request, scratch_left, sync_source, write,
};

/// The two files a round-trip case carries across.
const FIRST: &[u8] = b"the first file's bytes";
const SECOND: &[u8] = b"a second file, in a folder below";

/// A folder one device synced arrives, byte for byte, in another device's folder.
///
/// This is the round trip the whole path exists to make, and it is asserted from
/// what is on disk rather than from what the call returned. The target device
/// starts with an empty catalog, so its catch-up is a real restore-and-replay
/// (spec: CK-9, RV-1) — and everything after it follows from control state alone:
/// the committed Keyring opens under a purpose key derived from the Master Key,
/// the envelope it maps each Container to unwraps against that Container's own
/// ID, and the object decodes to the file that was on the other device's disk
/// (spec: RV-2, RV-3, KL-7, FM-14).
///
/// The Entry's modification time travels with it, and so does the device's claim
/// to have placed the file: fetching is the second way a device materializes an
/// Entry, so from here on this device may report the file as deleted if it goes
/// missing and may offer a change to it back to the Library (spec: EP-10,
/// EP-11).
pub async fn a_second_device_fetches_a_synced_folder(fixture: &FetchUnderTest) {
    let store = fixture.store();
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    let source_first = fixture.source_folder().join("a.jpg");
    write(fixture.source_folder(), "a.jpg", FIRST).await;
    write(fixture.source_folder(), "below/b.png", SECOND).await;
    let synced = sync_source(fixture, &keys, 1).await;
    assert_eq!(
        synced.added.len(),
        2,
        "one Container per file (spec: PK-15)"
    );

    let outcome = fetch_folders(request(store, fixture.target(), &keys, 2))
        .await
        .unwrap_or_else(|error| panic!("a fetch by a second device must succeed: {error}"));

    assert_eq!(
        outcome.fetched,
        vec![entry_path("a.jpg"), entry_path("below/b.png")],
        "both Entries, in the order the Library puts them in (spec: EP-3)",
    );
    assert_eq!(
        outcome.containers.len(),
        2,
        "the fetch unit is a whole Container (spec: PK-16)",
    );
    assert_eq!(outcome.skipped, 0);
    assert!(
        outcome.surfaced.is_empty(),
        "nothing was in the way, so nothing was declined (spec: EP-11)",
    );
    assert!(outcome.locked.is_empty());

    // What is actually on this device's disk, which is the only thing a fetch is
    // worth.
    let placed = fixture.target_folder().join("a.jpg");
    let below = fixture.target_folder().join("below/b.png");
    assert_eq!(read(&placed).await, FIRST);
    assert_eq!(read(&below).await, SECOND);

    let entry = entry_at(fixture.target(), "a.jpg").await.entry;
    let (size, mtime) = observed(&placed).await;
    assert_eq!(size, entry.size);
    assert_eq!(
        mtime, entry.mtime,
        "the placed file carries the Entry's own modification time (spec: FM-9, EP-11)",
    );
    assert_eq!(
        observed(&source_first).await.1,
        entry.mtime,
        "which is the time the file had on the device that synced it",
    );

    let local = fixture
        .target()
        .local_entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the target catalog for a local row must succeed")
        .expect("this device placed the file, so it has a row for it");
    assert_eq!(local.state, LocalEntryState::Present);
    assert_eq!(local.observation.size, entry.size);
    assert_eq!(local.observation.mtime, entry.mtime);

    assert_eq!(
        scratch_left(fixture.target_folder()).await,
        0,
        "a placed file leaves no temporary one behind (spec: EP-11)",
    );
}

/// A second fetch of an untouched folder places nothing and reads no Container.
///
/// The device's own materialization record matches the file on disk, so the file
/// *is* the Entry and there is nothing to fetch (spec: EP-10, EP-11). Which is a
/// claim about a cost as much as about an outcome — an Entry reported skipped
/// could still have been pulled down and discarded — so the reads are counted
/// rather than inferred. The stamp the first run set is what makes the cheap
/// comparison answer: a file left with the time it was written would look
/// changed to every later run.
pub async fn a_repeated_fetch_skips_everything_and_reads_no_container(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    write(fixture.source_folder(), "a.jpg", FIRST).await;
    write(fixture.source_folder(), "below/b.png", SECOND).await;
    sync_source(fixture, &keys, 1).await;

    let first = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2))
        .await
        .expect("a first fetch must succeed");
    assert_eq!(first.fetched.len(), 2);

    let counting = CountingStore::around(fixture.store());
    let second = fetch_folders(request(&counting, fixture.target(), &keys, 3))
        .await
        .expect("a second fetch of an untouched folder must succeed");

    assert!(second.fetched.is_empty(), "there was nothing to place");
    assert!(
        second.containers.is_empty(),
        "no Container was fetched a second time",
    );
    assert_eq!(second.skipped, 2);
    assert!(second.surfaced.is_empty());

    assert!(
        counting.listings() >= 1,
        "the run still read the Library's head (spec: CK-9)",
    );
    assert_eq!(
        counting.reads(),
        0,
        "and read no object at all: with nothing selected there is no Keyring \
         to open either",
    );
    assert_eq!(read(&fixture.target_folder().join("a.jpg")).await, FIRST);
}
