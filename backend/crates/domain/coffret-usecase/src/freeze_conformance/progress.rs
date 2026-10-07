use crate::freeze::freeze_folder;
use crate::freeze_conformance::fixtures::{filler, keys, map, request, write, TARGET};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;
use crate::freeze_conformance::slow_store::{SlowStore, PIECES};
use crate::progress::{ByteCount, Phase, Step};
use crate::recorded_progress::Recording;
use crate::upload::REPORT_EVERY;

/// A Pack on its way up is reported part way, in the bytes Storage has taken,
/// and the commit that follows it is reported as a phase of its own.
///
/// The store here pulls the body in pieces with a pause before each one longer
/// than the run's reporting period, so at least one report has to fall while
/// some of the Pack has gone and some has not. Every step of the phase counts
/// against the bytes of the whole phase, which is the one Pack: it starts at
/// nothing, never goes back on an upload that is not retried, and ends at all
/// of it.
pub async fn a_pack_on_its_way_up_is_seen_part_way(fixture: &FreezeUnderTest) {
    let store = SlowStore::around(fixture.store(), REPORT_EVERY * 2);
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "books/vol-1/001.png",
        &filler(200, 1),
    );

    let watching = Recording::default();
    let outcome =
        freeze_folder(request(&store, index, &keys, fixture.fs(), TARGET, 1).watched_by(&watching))
            .await
            .unwrap_or_else(|error| panic!("a watched freeze must succeed: {error}"));
    assert_eq!(outcome.packs.len(), 1, "one file is one Pack");

    let uploading: Vec<Step> = watching
        .steps()
        .into_iter()
        .filter(|step| step.phase == Phase::Uploading)
        .collect();
    let bytes: Vec<ByteCount> = uploading
        .iter()
        .map(|step| {
            step.bytes
                .unwrap_or_else(|| panic!("every step of the upload counts bytes: {step:?}"))
        })
        .collect();
    let total = bytes[0].total;
    assert!(total > 0, "a Pack is not empty");
    assert!(
        bytes.iter().all(|count| count.total == total),
        "the phase's total is known before the first byte and does not change: {bytes:?}",
    );
    assert_eq!(
        uploading
            .first()
            .map(|step| (step.done, step.total, bytes[0].done)),
        Some((0, Some(1), 0)),
        "the phase starts at nothing",
    );
    assert_eq!(
        uploading.last().map(|step| (step.done, step.total)),
        Some((1, Some(1))),
        "and ends with its one Pack",
    );
    assert_eq!(bytes.last().map(|count| count.done), Some(total));
    assert!(
        bytes.windows(2).all(|pair| pair[0].done <= pair[1].done),
        "an upload that is not retried never goes back: {bytes:?}",
    );

    let part_way: Vec<&Step> = uploading
        .iter()
        .filter(|step| {
            step.bytes
                .is_some_and(|count| count.done > 0 && count.done < count.total)
        })
        .collect();
    assert!(
        !part_way.is_empty(),
        "a put pulled in {PIECES} pieces is seen part way: {uploading:?}",
    );
    assert!(
        part_way.iter().all(|step| step.done == 0),
        "what is part way is the Pack in flight, which has not finished: {part_way:?}",
    );

    // Once the Pack is stored the run is committing, and says so rather than
    // going on reading as an upload at its last byte: the suite keeps two
    // Keyring replicas, and the head makes three objects (spec: CP-8, CP-2).
    let steps = watching.steps();
    let committing_from = steps
        .iter()
        .position(|step| step.phase == Phase::Committing)
        .unwrap_or_else(|| panic!("a freeze that packed something commits it: {steps:?}"));
    assert_eq!(
        &steps[committing_from..],
        [0, 1, 2, 3].map(|done| Step::new(Phase::Committing, done, 3)),
        "the commit comes after the upload and counts the objects it stores",
    );
    assert_eq!(
        steps[committing_from - 1],
        *uploading.last().expect("the upload was reported"),
        "the commit follows straight on from the upload's last step",
    );
}
