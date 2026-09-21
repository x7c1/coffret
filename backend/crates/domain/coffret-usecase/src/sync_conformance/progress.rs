use crate::progress::{Phase, Step};
use crate::recorded_progress::Recording;
use crate::sync::sync_folders;
use crate::sync_conformance::fixtures::{keys, map, request, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A run says which phase it is in, and counts the two phases it can count.
///
/// A sync is the longest a person waits on one call: it catches the catalog up
/// to the Library's head, settles what an interrupted run left, walks the
/// mapped folders, encodes what it found and sends it. Until it is finished it
/// returns nothing, and a caller with nothing to show cannot tell a transfer
/// that is working from one that has stopped.
///
/// Two files make two Containers (spec: PK-15), so the encoding and the sending
/// each walk 0, 1, 2 of 2 — before the first unit and after every one of them,
/// so that a caller has a line up while the first and perhaps largest object
/// travels. A run that reported only at the end, or only per phase, would say
/// something else.
///
/// The three phases before them can say no total at all: what a catch-up has to
/// replay is known only as it is replayed, what an interrupted run left is
/// known only by looking, and the scan is the very thing that counts the files.
/// They say that they have begun and nothing more — which is all a caller can
/// render, and a run that stayed quiet until it could count would be silent
/// through the stretch this exists for.
///
/// The second run is the same story with nothing in it. The folders have not
/// changed, so there is nothing to encode and nothing to send, and the phases
/// that can count say `0` of `0` rather than going missing: a caller shows
/// nothing for those and still has the three phases before them, which is the
/// difference between a quiet run and a run that has stopped.
pub async fn a_run_says_which_phase_it_is_in_and_counts_the_ones_it_can(fixture: &SyncUnderTest) {
    let keys = keys();
    let store = fixture.store();
    let index = fixture.index();

    map(fixture, None).await;
    write(fixture.fs(), fixture.folder(), "a.jpg", b"a photo");
    write(fixture.fs(), fixture.folder(), "below/b.png", b"a page");

    let watching = Recording::default();
    let carried = sync_folders(request(store, index, &keys, fixture.fs(), 1).watched_by(&watching))
        .await
        .unwrap_or_else(|error| panic!("a watched sync must succeed: {error}"));

    assert_eq!(carried.added.len(), 2, "both files must go up");
    assert_eq!(
        watching.steps(),
        [
            Step::begun(Phase::CatchingUp),
            Step::begun(Phase::Reconciling),
            Step::begun(Phase::Scanning),
            Step::new(Phase::Packing, 0, 2),
            Step::new(Phase::Packing, 1, 2),
            Step::new(Phase::Packing, 2, 2),
            Step::new(Phase::Uploading, 0, 2),
            Step::new(Phase::Uploading, 1, 2),
            Step::new(Phase::Uploading, 2, 2),
        ],
        "a run says which phase it is in, and counts the ones it can count",
    );
    for step in watching.steps() {
        let counted = matches!(step.phase, Phase::Packing | Phase::Uploading);
        assert_eq!(
            step.total.is_some(),
            counted,
            "only a phase that knows its size may claim one: {step:?}",
        );
    }

    let watching_again = Recording::default();
    let again =
        sync_folders(request(store, index, &keys, fixture.fs(), 2).watched_by(&watching_again))
            .await
            .unwrap_or_else(|error| panic!("a second watched sync must succeed: {error}"));

    assert!(
        again.added.is_empty(),
        "nothing changed under the folder, so nothing goes up",
    );
    assert_eq!(
        watching_again.steps(),
        [
            Step::begun(Phase::CatchingUp),
            Step::begun(Phase::Reconciling),
            Step::begun(Phase::Scanning),
            Step::new(Phase::Packing, 0, 0),
            Step::new(Phase::Uploading, 0, 0),
        ],
        "a run with nothing to carry still says what it is doing while it looks",
    );
}
