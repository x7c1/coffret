use coffret_format::generate_container_id;

use crate::device_state::{Mapping, RootMarkerId};
use crate::local_operation::LocalOperation;
use crate::scratch;
use crate::sync::sync_folders;
use crate::sync_conformance::fixtures::{keys, request, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A scratch a local writer that is gone left in a mapped folder is removed by
/// the next sync, and nothing else carrying the prefix is.
///
/// A writer killed before its rename leaves a file under coffret's scratch
/// prefix that the scan steps over, so nothing would ever take it out of the
/// folder (spec: EP-11). The sync removes it, as what this device wrote for its
/// own purposes (spec: OC-8) — at the top of the root and below it. A scratch a
/// live writer in this process holds is left to that writer, and a name that
/// merely carries the prefix is the person's own and is left alone too. None of
/// them is committed.
pub async fn a_scratch_an_interrupted_write_left_is_removed_by_the_next_sync(
    fixture: &SyncUnderTest,
) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map_registered(fixture).await;

    write(fixture.fs(), fixture.folder(), "a.jpg", b"the file's bytes");
    let left = scratch::incoming_name().as_str().to_owned();
    let fetched = scratch::name(generate_container_id().expect("the OS CSPRNG is available"))
        .as_str()
        .to_owned();
    let stale = [
        write(fixture.fs(), fixture.folder(), &left, b"half an upload"),
        write(
            fixture.fs(),
            fixture.folder(),
            &format!("album/{fetched}"),
            b"half a fetch",
        ),
    ];
    let live = scratch::incoming_name();
    let writing = write(
        fixture.fs(),
        fixture.folder(),
        live.as_str(),
        b"an upload still arriving",
    );
    let theirs = write(
        fixture.fs(),
        fixture.folder(),
        ".coffret-fetch-notes.txt",
        b"the person's own",
    );

    let outcome = sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .expect("a sync over a folder holding scratches must succeed");

    let commit = outcome.commit.expect("the file is worth a commit");
    assert_eq!(commit.record.additions().len(), 1, "no scratch is an Entry");
    for path in &stale {
        assert!(
            !fixture.fs().holds(path),
            "a scratch nothing holds is removed (spec: OC-8): {path:?}",
        );
    }
    assert!(
        fixture.fs().holds(&writing),
        "a scratch a live writer holds is its writer's",
    );
    assert!(
        fixture.fs().holds(&theirs),
        "a name that only carries the prefix is not coffret's to remove",
    );

    // Once its writer lets it go without a rename, the next run takes it too.
    drop(live);
    let again = sync_folders(request(store, index, &keys, fixture.fs(), 2))
        .await
        .expect("a second sync must succeed");
    assert!(again.commit.is_none(), "nothing new to commit");
    assert!(!fixture.fs().holds(&writing));
    assert!(fixture.fs().holds(&theirs));
}

/// A scratch whose removal is refused stays, and the next sync retries it.
///
/// The removal is idempotent and nothing in the run depends on it, so a
/// refusal is reported and does not fail the sync, and the run after it meets
/// the same scratch and removes it (spec: OC-8).
pub async fn a_refused_scratch_removal_is_retried_by_the_next_sync(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map_registered(fixture).await;

    let left = scratch::incoming_name().as_str().to_owned();
    let path = write(fixture.fs(), fixture.folder(), &left, b"half an upload");

    // The folder holds nothing to spool, so the removal is the run's first.
    fixture.fs().fail_on(LocalOperation::Removing, 1);
    sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .expect("a refused scratch removal does not fail the sync");
    assert!(fixture.fs().holds(&path), "the refusal leaves the scratch");

    sync_folders(request(store, index, &keys, fixture.fs(), 2))
        .await
        .expect("the retry must succeed");
    assert!(!fixture.fs().holds(&path), "the next run removes it");
}

/// Maps the fixture's folder onto the Library root with a marker in the root
/// and the same identity expected of it.
///
/// Removing a scratch is a write into the mapped folder, and a write is only
/// ever made through a root that vouches for itself (spec: EP-13).
async fn map_registered(fixture: &SyncUnderTest) {
    let registered = RootMarkerId::from_bytes([0x5c; RootMarkerId::BYTE_LEN]);
    fixture.fs().plant_marker(fixture.folder(), &registered);
    fixture
        .index()
        .set_mapping(Mapping::new(None, fixture.folder().to_path_buf()).expecting(registered))
        .await
        .expect("recording a mapping must succeed");
}
