use crate::entry_paths::entry_path;
use crate::fetch::fetch_folders;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{keys, map, request, sync_source, write};
use crate::progress::{Phase, Step};
use crate::recorded_progress::Recording;

/// A run says how much it has to do and how much of it is done.
///
/// A fetch of a folder is minutes of silence otherwise: the call returns an
/// outcome when it is finished and says nothing on the way, which is
/// indistinguishable from a transfer that has stopped. What it reports is
/// Containers rather than bytes — that is the unit it loops over, and the one
/// the port lets it count — and the first step is said *before* the first
/// object travels, so a caller has something up while the longest single wait
/// of the run happens.
///
/// Two files make two Containers (spec: PK-15), so the run walks 0, 1, 2 of 2
/// and a run that reported only at the end, or only per file, would say
/// something else.
///
/// The counted phase is not the whole of it. Everything before the first
/// Container — catching the catalog up to the Library's head, then deciding
/// where each current Entry would go — is a phase that cannot say how much work
/// it holds until it has done it, and on a device that has just joined the
/// catch-up is the longest part of the run. Those say that they have begun and
/// nothing more, which is exactly what a caller can show, and a run that said
/// nothing until it could count would be silent through the stretch this exists
/// for.
pub async fn a_run_says_how_far_through_the_containers_it_is(fixture: &FetchUnderTest) {
    let keys = keys();
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;

    write(fixture.fs(), fixture.source_folder(), "a.jpg", b"a photo");
    write(
        fixture.fs(),
        fixture.source_folder(),
        "below/b.png",
        b"a page",
    );
    sync_source(fixture, &keys, 1).await;

    let watching = Recording::default();
    let outcome = fetch_folders(request(fixture.store(), fixture, &keys, 2).watched_by(&watching))
        .await
        .unwrap_or_else(|error| panic!("a watched fetch must succeed: {error}"));

    assert_eq!(outcome.fetched.len(), 2, "both files must be placed");
    assert_eq!(
        watching.steps(),
        [
            // Said before anything is read, and carrying no total: what a
            // catch-up has to replay is known only as it is replayed.
            Step::begun(Phase::CatchingUp),
            Step::begun(Phase::Scanning),
            Step::new(Phase::Fetching, 0, 2),
            Step::new(Phase::Fetching, 1, 2),
            Step::new(Phase::Fetching, 2, 2),
        ],
        "a run says which phase it is in, and counts the one phase it can count",
    );
    for step in watching.steps() {
        let counted = matches!(step.phase, Phase::Fetching);
        assert_eq!(
            step.total.is_some(),
            counted,
            "only the phase that knows its size may claim one: {step:?}",
        );
    }
}

/// A device that maps nothing is told apart from a prefix that holds nothing.
///
/// The two produce the same counts — nothing fetched, no Container read,
/// nothing skipped — and they are different states. A prefix that names no
/// current Entry is an ordinary empty answer about the Library. A device with no
/// mapping at all has nowhere to put anything, so it would answer that way about
/// every prefix there is, and somebody who has just joined a Library would read
/// it as everything already being here (spec: EP-9).
///
/// So the run says how many mappings it worked from, and a caller that shows the
/// counts alone has the one fact that tells the two apart.
pub async fn a_device_that_maps_nothing_is_told_apart_from_an_empty_prefix(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    write(fixture.fs(), fixture.source_folder(), "a.jpg", b"a photo");
    sync_source(fixture, &keys, 1).await;

    // The target device has recorded no mapping at all yet, which is what a
    // device that has only just joined looks like.
    let unmapped = fetch_folders(request(fixture.store(), fixture, &keys, 2))
        .await
        .unwrap_or_else(|error| {
            panic!("a fetch by a device that maps nothing must succeed: {error}")
        });
    assert_eq!(unmapped.mappings, 0, "the device has recorded no mapping");
    assert!(
        unmapped.fetched.is_empty(),
        "there is nowhere to place anything"
    );

    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;
    let elsewhere = fetch_folders(
        request(fixture.store(), fixture, &keys, 3).under(entry_path("nothing-of-that-name")),
    )
    .await
    .unwrap_or_else(|error| panic!("a fetch under an empty prefix must succeed: {error}"));

    assert_eq!(
        elsewhere.mappings, 1,
        "the device maps a folder; the prefix is what held nothing",
    );
    assert!(
        elsewhere.fetched.is_empty(),
        "no Entry stands under that prefix",
    );
}
