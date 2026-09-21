use crate::sync::sync_folders;
use crate::sync_conformance::fixtures::{keys, map, request, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A device that maps nothing is told apart from folders with nothing new.
///
/// The two produce the same counts — nothing added, nothing replaced, nothing
/// committed — and they are different states. A run that found every file
/// already current is the ordinary second sync of a folder. A device with no
/// mapping at all has nothing inside the Library's scope, so it would answer
/// that way whatever is on its disk, and somebody who has just joined a Library
/// would read it as everything already being backed up (spec: EP-9).
///
/// So the run says how many mappings it worked from, and a caller that shows
/// the counts alone has the one fact that tells the two apart. It is the same
/// fact a fetch reports for the same reason, because the person reading it is
/// the same person: whoever has just joined tries one of the two commands
/// first, and which one they try must not decide whether they are told.
pub async fn a_device_that_maps_nothing_is_told_apart_from_a_folder_with_nothing_new(
    fixture: &SyncUnderTest,
) {
    let keys = keys();
    let store = fixture.store();
    let index = fixture.index();

    // Files on disk and no mapping at all, which is what a device that has only
    // just joined looks like.
    write(fixture.fs(), fixture.folder(), "a.jpg", b"a photo");

    let unmapped = sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .unwrap_or_else(|error| {
            panic!("a sync by a device that maps nothing must succeed: {error}")
        });
    assert_eq!(unmapped.mappings, 0, "the device has recorded no mapping");
    assert!(
        unmapped.added.is_empty(),
        "nothing on this disk is inside the Library's scope",
    );
    assert!(
        unmapped.commit.is_none(),
        "a run with nothing to upload commits nothing (spec: CP-1)",
    );

    map(fixture, None).await;
    let carried = sync_folders(request(store, index, &keys, fixture.fs(), 2))
        .await
        .unwrap_or_else(|error| panic!("a sync of a mapped folder must succeed: {error}"));
    assert_eq!(carried.mappings, 1, "the device maps a folder now");
    assert_eq!(carried.added.len(), 1, "and the file it holds went up");

    // The ordinary second run: the same empty counts as the first run above,
    // and the one number that says why they are empty differs.
    let again = sync_folders(request(store, index, &keys, fixture.fs(), 3))
        .await
        .unwrap_or_else(|error| panic!("a second sync of the folder must succeed: {error}"));
    assert_eq!(
        again.mappings, 1,
        "the device maps a folder; the folder is what held nothing new",
    );
    assert!(again.added.is_empty());
    assert!(again.commit.is_none());
}
