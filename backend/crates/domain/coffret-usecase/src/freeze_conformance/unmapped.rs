use crate::freeze_conformance::fixtures::{freeze, keys, map, write, TARGET};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;

/// A device that maps nothing is told apart from a folder already packed.
///
/// The two produce the same counts — no Pack, nothing absorbed, nothing
/// committed — and they are different states. A run that found every file
/// already in a Pack is the ordinary second freeze of a folder (spec: PK-2). A
/// device with no mapping at all has nothing inside the Library's scope, so it
/// would answer that way whatever is on its disk, and somebody who has just
/// joined a Library would read it as everything already being packed
/// (spec: EP-9).
///
/// So the run says how many mappings it worked from, and a caller that shows
/// the counts alone has the one fact that tells the two apart. It is the same
/// fact a sync and a fetch report for the same reason: whoever has just joined
/// tries one of the three commands first, and which one they try must not
/// decide whether they are told.
pub async fn a_device_that_maps_nothing_is_told_apart_from_a_folder_already_packed(
    fixture: &FreezeUnderTest,
) {
    let keys = keys();

    // Files on disk and no mapping at all, which is what a device that has only
    // just joined looks like.
    write(fixture.fs(), fixture.source_folder(), "a.jpg", b"a photo");

    let unmapped = freeze(fixture, &keys, TARGET, 1).await;
    assert_eq!(unmapped.mappings, 0, "the device has recorded no mapping");
    assert!(
        unmapped.packs.is_empty(),
        "nothing on this disk is inside the Library's scope",
    );
    assert_eq!(unmapped.packed_already, 0);
    assert!(
        unmapped.commit.is_none(),
        "a run that selected nothing commits nothing (spec: CP-1)",
    );

    map(fixture.source(), None, fixture.source_folder()).await;
    let packed = freeze(fixture, &keys, TARGET, 2).await;
    assert_eq!(packed.mappings, 1, "the device maps a folder now");
    assert_eq!(
        packed.frozen_entries(),
        1,
        "and the file it holds was packed"
    );

    // The ordinary second run: the same empty counts as the first run above,
    // and the one number that says why they are empty differs.
    let again = freeze(fixture, &keys, TARGET, 3).await;
    assert_eq!(
        again.mappings, 1,
        "the device maps a folder; the folder is what was already packed",
    );
    assert!(again.packs.is_empty());
    assert_eq!(
        again.packed_already, 1,
        "the file is in a Pack (spec: PK-2)"
    );
    assert!(again.commit.is_none());
}
