use crate::device_state::LocalEntryState;
use crate::entry_paths::entry_path;
use crate::fetch::{fetch_folders, Surfaced};
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    at, exists, keys, map, read, request, scratch_left, sync_source, write,
};

/// What the source device puts in the Library in these cases.
const HELD: &[u8] = b"what the Library holds";

/// A file this device did not place is reported and left exactly as it is.
///
/// The device has no materialization record for the path, so what stands there is
/// not its copy of anything — it may be a source file nobody has synced yet, and
/// overwriting it would destroy content the Library has never held. The one state
/// a fetch may claim is an empty path (spec: EP-10, EP-11), so this one is
/// reported and passed over.
///
/// Reported, not skipped: a run that returned success with this file untouched
/// and said nothing would leave the user believing the folder is a copy of the
/// Library.
pub async fn a_foreign_file_is_surfaced_and_left_untouched(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    write(fixture.source_folder(), "a.jpg", HELD).await;
    write(
        fixture.source_folder(),
        "b.jpg",
        b"a file with a clear path",
    )
    .await;
    sync_source(fixture, &keys, 1).await;

    let mine = b"bytes this device happens to have at that path".as_slice();
    let occupied = write(fixture.target_folder(), "a.jpg", mine).await;

    let outcome = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2))
        .await
        .unwrap_or_else(|error| panic!("a fetch meeting a foreign file must succeed: {error}"));

    assert_eq!(
        outcome.fetched,
        vec![entry_path("b.jpg")],
        "the path nothing occupied was placed, and only that one",
    );
    assert_eq!(
        outcome.surfaced,
        vec![Surfaced::ForeignFile {
            path: entry_path("a.jpg"),
        }],
    );
    assert_eq!(read(&occupied).await, mine, "byte for byte as it was");
    assert!(
        fixture
            .target()
            .local_entry_at(&entry_path("a.jpg"))
            .await
            .expect("asking the target catalog for a local row must succeed")
            .is_none(),
        "a fetch that placed nothing invents no claim to have placed it (spec: EP-10)",
    );
    assert_eq!(scratch_left(fixture.target_folder()).await, 0);
}

/// A file this device placed and has since changed is reported and left alone.
///
/// The record says the device materialized the path, and the file no longer
/// matches what it wrote down — which makes this a pending local change the sync
/// flow owns, either a modification to offer the Library or a deletion to report
/// (spec: EP-10). A fetch that wrote over it would quietly undo work, so it
/// reports and stops (spec: EP-11).
pub async fn a_locally_changed_file_is_surfaced_and_left_untouched(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    write(fixture.source_folder(), "a.jpg", HELD).await;
    sync_source(fixture, &keys, 1).await;

    let first = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2))
        .await
        .expect("a first fetch must succeed");
    assert_eq!(first.fetched.len(), 1);

    let changed = b"what this device did with it afterwards".as_slice();
    let placed = write(fixture.target_folder(), "a.jpg", changed).await;

    let outcome = fetch_folders(request(fixture.store(), fixture.target(), &keys, 3))
        .await
        .unwrap_or_else(|error| panic!("a fetch meeting a local change must succeed: {error}"));

    assert!(outcome.fetched.is_empty());
    assert!(
        outcome.containers.is_empty(),
        "nothing was selected, so nothing was fetched",
    );
    assert_eq!(
        outcome.surfaced,
        vec![Surfaced::LocallyChanged {
            path: entry_path("a.jpg"),
        }],
    );
    assert_eq!(read(&placed).await, changed, "byte for byte as it was");
    assert_eq!(scratch_left(fixture.target_folder()).await, 0);
}

/// A deletion this device witnessed is reported and the file is not put back.
///
/// The row records that this device had the file and lost it, which is the one
/// shape a local deletion takes (spec: EP-10). Restoring it is an explicit
/// operation — the mirror of propagating the deletion on the sync side — and a
/// fetch that inferred it from the row would undo a deletion the user made.
///
/// The finding comes back on every later run, because nothing about the row
/// changed.
pub async fn a_witnessed_deletion_is_surfaced_and_not_refetched(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    write(fixture.source_folder(), "a.jpg", HELD).await;
    sync_source(fixture, &keys, 1).await;

    let placed = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2))
        .await
        .expect("a first fetch must succeed");
    assert_eq!(placed.fetched.len(), 1);

    // The device notices the file is gone. A scan is what would do this in
    // production; the row it leaves is what the fetch reads.
    tokio::fs::remove_file(fixture.target_folder().join("a.jpg"))
        .await
        .expect("removing a placed file must succeed");
    fixture
        .target()
        .mark_absent(&entry_path("a.jpg"), at(3))
        .await
        .expect("recording a witnessed deletion must succeed");

    let outcome = fetch_folders(request(fixture.store(), fixture.target(), &keys, 4))
        .await
        .unwrap_or_else(|error| {
            panic!("a fetch meeting a witnessed deletion must succeed: {error}")
        });

    assert!(outcome.fetched.is_empty());
    assert!(outcome.containers.is_empty());
    assert_eq!(
        outcome.surfaced,
        vec![Surfaced::WitnessedDeletion {
            path: entry_path("a.jpg"),
        }],
    );
    assert!(
        !exists(&fixture.target_folder().join("a.jpg")).await,
        "the file stays gone: putting it back is an explicit operation",
    );

    let local = fixture
        .target()
        .local_entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the target catalog for a local row must succeed")
        .expect("the row outlives the file it was made for");
    assert_eq!(
        local.state,
        LocalEntryState::Absent,
        "so every later run reports the same finding",
    );

    let again = fetch_folders(request(fixture.store(), fixture.target(), &keys, 5))
        .await
        .expect("a third fetch must succeed");
    assert_eq!(again.surfaced, outcome.surfaced);
}
