//! What a deletion says about itself while it runs.

use crate::delete::delete_entries;
use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::delete_conformance::fixtures::{
    folder, keys, map_registered, paths, request, sync, write,
};
use crate::progress::{Phase, Step};
use crate::recorded_progress::Recording;

/// EP-15: once a deletion's commit has landed, removing this device's own
/// copies of what it deleted is a phase of its own, after
/// [`Committing`](Phase::Committing) and the last thing the run says.
///
/// Each copy is read and hashed before it is discarded, so on a large folder
/// this takes long enough after the head that a run still standing at
/// `committing` would read as a commit that had stalled. It cannot count the
/// copies — which of the deleted Entries this device holds one of is learned by
/// looking — so it says that it has begun and nothing more.
///
/// A deletion handed no folders to reach its copies through never enters it:
/// it has nothing of this device's to remove.
pub async fn a_deletion_says_it_is_removing_this_devices_copies_after_committing(
    fixture: &DeleteUnderTest,
) {
    let index = fixture.index();
    map_registered(fixture).await;
    write(fixture, "album/a.jpg", b"the first file");
    write(fixture, "album/b.jpg", b"the second file");
    write(fixture, "c.jpg", b"a file beside the folder");
    sync(fixture, 1).await;

    let keys = keys();
    let watching = Recording::default();
    let outcome = delete_entries(
        request(fixture.store(), index, fixture, &keys, folder("album"), 2)
            .with_mapped_copies(fixture.fs(), fixture.fs())
            .watched_by(&watching),
    )
    .await
    .unwrap_or_else(|error| panic!("a watched deletion must succeed: {error}"));
    assert!(outcome.commit.is_some(), "the deletion committed");

    let steps = watching.steps();
    let committed = steps
        .iter()
        .rposition(|step| step.phase == Phase::Committing)
        .expect("a deletion that committed says it was committing");
    assert_eq!(
        &steps[committed + 1..],
        [Step::begun(Phase::RemovingCopies)],
        "after the commit, the run says it is removing this device's copies, once and last",
    );

    let watching = Recording::default();
    delete_entries(
        request(fixture.store(), index, fixture, &keys, paths(&["c.jpg"]), 3).watched_by(&watching),
    )
    .await
    .unwrap_or_else(|error| panic!("a watched deletion must succeed: {error}"));
    assert!(
        watching
            .steps()
            .iter()
            .all(|step| step.phase != Phase::RemovingCopies),
        "a deletion handed no folders has no copies of this device's to remove",
    );
}
