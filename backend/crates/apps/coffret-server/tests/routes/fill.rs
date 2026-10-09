//! The fill: the rest of the folder somebody opened a file in, brought over in
//! the background, one folder at a time, with the latest one asked for winning.

use coffret_logging::testing::CapturedLogs;
use serde_json::json;
use tracing::Level;

use crate::support::{declined, fill, json as body_of, states, without_server, Served};

// Whoever opened page one is going to read page two: the folder holding the
// Entry that had to be fetched is brought over behind the request, without
// anything being clicked again (spec: EP-10, EP-11).
#[tokio::test]
async fn opening_a_file_brings_the_rest_of_its_folder_over() {
    let served = Served::library().await;

    let answer = served.get("/api/file?path=albums/cover.png").await;
    assert_eq!(answer.status(), 200);

    // Named the moment the request is answered, whatever the fill has managed
    // by then: what arms it is the fetch, and the fetch is over.
    let (_, armed) = body_of(served.get("/api/work").await).await;
    assert_eq!(fill(&armed)["folder"], "albums");

    served.fill_idle().await;
    let (_, done) = body_of(served.get("/api/work").await).await;
    assert_eq!(fill(&done)["folder"], "albums");
    assert_eq!(fill(&done)["status"], "done");
    assert_eq!(
        (fill(&done)["done"].as_u64(), fill(&done)["total"].as_u64()),
        (Some(2), Some(2)),
        "the two rows the listing still called remote, and no others: {done}",
    );
    assert_eq!(declined(fill(&done)), Vec::<(String, String)>::new());
    assert_eq!(
        fill(&done)["findings"],
        json!([]),
        "a Keyring nothing stepped over is not mentioned",
    );
    assert_eq!(fill(&done)["stopped"], serde_json::Value::Null);

    // The listing stays the one answer about what is on this device, and it now
    // says the whole folder is (spec: EP-10).
    let (_, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(
        states(&listing),
        [
            ("caf\u{e9}.jpg".to_owned(), "present".to_owned()),
            ("cover.png".to_owned(), "present".to_owned()),
            ("notes.txt".to_owned(), "present".to_owned()),
        ],
    );
    // One folder down is a folder of its own and is left alone: what was opened
    // says which folder somebody is reading, not which subtree.
    let (_, deeper) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert!(states(&deeper).iter().all(|(_, state)| state == "remote"));
}

// EP-11: a declined Entry is a finding about that Entry and not the fill's
// failure. It is recorded with what the file route would have said about it —
// so the row can be marked without anybody clicking it — and the fill goes on
// to the next file, exactly as the command line's fetch does.
#[tokio::test]
async fn an_entry_the_fill_declines_is_reported_and_the_rest_still_arrive() {
    let served = Served::library().await;
    served.plant_locally("albums/cover.png", b"something of my own");

    assert_eq!(
        served.get("/api/file?path=albums/notes.txt").await.status(),
        200,
    );
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let fill = fill(&work);
    assert_eq!(fill["status"], "done", "a finding does not stop a fill");
    assert_eq!(
        (fill["done"].as_u64(), fill["total"].as_u64()),
        (Some(1), Some(2)),
    );
    assert_eq!(
        declined(fill),
        [("albums/cover.png".to_owned(), "surfaced".to_owned())],
    );
    assert_eq!(fill["declined"][0]["error"], "declined");
    assert_eq!(fill["declined"][0]["surfaced"], "ForeignFile");

    assert!(served.holds("albums/caf\u{e9}.jpg"), "the fill went on");
    assert_eq!(
        std::fs::read(served.local_path("albums/cover.png")).expect("the file is still there"),
        b"something of my own",
    );
}

// A Storage that has gone is the one thing a fill stops for: every further Entry
// would meet it identically, so it is reported once and the rest of the folder
// is left where it was. Taking the folder up again is the browser's to ask for,
// and it is the whole of what `POST /api/fill` is for.
#[tokio::test]
async fn storage_stops_a_fill_and_the_folder_can_be_taken_up_again() {
    let served = Served::library().await;

    // What one attempt costs against a Storage that refuses reads, measured
    // rather than assumed: this Entry's folder holds nothing else, so what is
    // spent is one Entry's worth and nothing follows it.
    served.halt_storage();
    let refused = served.get("/api/file?path=books/page-001.png").await;
    assert_eq!(refused.status(), 502);
    let one_attempt = served.refused_reads();
    assert!(one_attempt > 0, "an attempt reaches Storage");

    let armed = served.post("/api/fill?path=albums/2026").await;
    assert_eq!(armed.status(), 202);
    served.fill_idle().await;

    let (_, stopped) = body_of(served.get("/api/work").await).await;
    let fill_stopped = fill(&stopped);
    assert_eq!(fill_stopped["folder"], "albums/2026");
    assert_eq!(fill_stopped["status"], "stopped");
    assert_eq!(
        (
            fill_stopped["done"].as_u64(),
            fill_stopped["total"].as_u64()
        ),
        (Some(0), Some(2)),
        "what it set out to bring over, and none of it: {stopped}",
    );
    assert_eq!(fill_stopped["stopped"]["error"], "storage");
    assert_eq!(
        served.refused_reads() - one_attempt,
        one_attempt,
        "the fill stopped at the first Entry rather than trying the second",
    );

    // The rows are untouched, which is what makes the retry worth offering.
    let (_, waiting) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert!(states(&waiting).iter().all(|(_, state)| state == "remote"));

    served.resume_storage();
    assert_eq!(
        served.post("/api/fill?path=albums/2026").await.status(),
        202
    );
    served.fill_idle().await;

    let (_, finished) = body_of(served.get("/api/work").await).await;
    assert_eq!(fill(&finished)["status"], "done");
    assert_eq!(
        (
            fill(&finished)["done"].as_u64(),
            fill(&finished)["total"].as_u64()
        ),
        (Some(2), Some(2)),
    );
    let (_, filled) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert!(states(&filled).iter().all(|(_, state)| state == "present"));
}

// Latest wins. Somebody who armed a second folder has moved on, so the fill
// follows them there rather than finishing what they left — and the folder it
// left is not taken up again on its own.
//
// Two armings with nothing awaited between them, which is the one way to say
// this as a case: anything awaited would let the worker run, and what it
// managed first would be the scheduler's answer rather than the rule's.
#[tokio::test]
async fn a_fill_is_superseded_by_the_folder_armed_after_it() {
    let served = Served::library().await;

    served.arm_fill("albums/2026");
    served.arm_fill("books");
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(fill(&work)["folder"], "books");
    assert_eq!(fill(&work)["status"], "done");
    assert!(served.holds("books/page-001.png"));

    let (_, left) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert!(
        states(&left).iter().all(|(_, state)| state == "remote"),
        "the folder that was left is not resumed on its own: {left}",
    );
}

// The run the case above never lets start is the run this one catches in the
// middle: the fill has taken its folder up and is inside Storage for the first
// of its two files when the second folder is armed, so the rule is met where it
// is written — between one Entry and the next — and the run it leaves says so.
//
// Held rather than raced. Storage takes the read and keeps it until the case
// lets go, so the second arming lands while the first run is provably under
// way, and what it came to is the rule's answer and not the scheduler's.
#[tokio::test]
async fn a_fill_under_way_is_superseded_between_one_entry_and_the_next() {
    let served = Served::library().await;
    let logs = CapturedLogs::capture();
    served.hold_storage();

    served.arm_fill("albums/2026");
    // No sleep and no guess: the read is counted as it arrives.
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
    served.arm_fill("books");
    served.release_storage();
    served.fill_idle().await;

    let outcomes: Vec<(String, String)> = logs
        .at(Level::INFO)
        .into_iter()
        .filter(|event| event.message() == "a folder was brought over")
        .map(|event| (event.field("outcome"), event.field("path_len")))
        .collect();
    assert_eq!(
        outcomes,
        [
            ("superseded".to_owned(), "albums/2026".len().to_string()),
            ("done".to_owned(), "books".len().to_string()),
        ],
        "the run that was left says it was left, and the one it left for finishes",
    );

    let (_, left) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert!(
        states(&left).iter().any(|(_, state)| state == "remote"),
        "the folder that was left is not finished on its own: {left}",
    );
    assert!(served.holds("books/page-001.png"));
}

// And the other half of that rule: a folder asked for by name waits its turn
// rather than taking the running one's place. Both are brought over, in the
// order they were asked for.
//
// The two are the same pair of buttons on the screen — one per folder a worker
// that died threw away — and latest wins between them would be the second press
// taking the first folder's line, its button and every trace of it away, with
// half of the folder brought over and nothing left saying so.
//
// Two askings with nothing awaited between them, for the reason the case above
// gives: anything awaited would let the worker finish the first, and the
// ordering would be the scheduler's answer rather than the rule's.
#[tokio::test]
async fn a_folder_asked_for_by_name_waits_behind_the_one_being_filled() {
    let served = Served::library().await;

    served.queue_fill("albums/2026");
    served.queue_fill("books");
    served.fill_idle().await;

    assert!(served.holds("albums/2026/spring.jpg"));
    assert!(served.holds("albums/2026/summer.jpg"));
    assert!(served.holds("books/page-001.png"));

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        fill(&work)["folder"],
        "books",
        "the second is the run on record, having run after the first rather than instead of it",
    );
    assert_eq!(fill(&work)["status"], "done");
    assert_eq!(fill(&work)["run"], 2, "two runs, not one superseding one");
}

// One Storage outage stops the folder being brought over and every folder queued
// behind it, and the worker takes the next one the moment the first returns. The
// run on record is then the last of them, and without this the ones before it
// would be on the wire nowhere at all: the line naming the folder, the Entries it
// declined and the offer of a second attempt would go together, unread, inside a
// tick.
#[tokio::test]
async fn a_fill_storage_stopped_is_still_named_once_the_next_folder_runs() {
    let served = Served::library().await;
    served.halt_storage();

    served.queue_fill("albums/2026");
    served.queue_fill("books");
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let latest = fill(&work);
    assert_eq!(latest["folder"], "books");
    assert_eq!(latest["status"], "stopped");

    let displaced = latest["displaced"].as_array().expect("an array");
    assert_eq!(displaced.len(), 1, "the folder the record was taken from");
    assert_eq!(displaced[0]["folder"], "albums/2026");
    assert_eq!(displaced[0]["status"], "stopped");
    assert_eq!(
        displaced[0]["stopped"]["error"], "storage",
        "with what stopped it, which is what the second attempt is offered from",
    );
    assert_eq!(
        displaced[0]["run"], 1,
        "and as the run it was, so a line put away does not take this one with it",
    );
    for list in ["waiting", "discarded", "displaced"] {
        assert!(
            displaced[0].get(list).is_none(),
            "the queue is the flow's and is said once, on the run the flow is on, \
             so a displaced run carries no `{list}`",
        );
    }

    // And the notice about a folder ends where somebody takes that folder up,
    // exactly as a folder the queue lost does. The run this press displaces in
    // its turn is the one that had stopped on record, which is owed the same
    // line for the same reason.
    served.resume_storage();
    let (status, armed) = body_of(served.post("/api/fill?path=albums/2026").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        fill(&armed)["folder"],
        "albums/2026",
        "the press is the run on record from the moment it is armed",
    );
    assert_eq!(
        fill(&armed)["displaced"]
            .as_array()
            .expect("an array")
            .iter()
            .map(|run| run["folder"].clone())
            .collect::<Vec<_>>(),
        ["books"],
        "the folder pressed is off the list, and the one it took the record from is on it",
    );
}

// A folder no mapping of this device reaches has nowhere to put a file
// (spec: EP-9), so there is nothing there to bring over — and the fill says so
// rather than asking Storage once per file to be told so once per file.
#[tokio::test]
async fn a_fill_of_a_folder_no_mapping_of_this_device_reaches_does_nothing() {
    let served = Served::mapping_only("albums").await;

    assert_eq!(served.post("/api/fill?path=books").await.status(), 202);
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(fill(&work)["folder"], "books");
    assert_eq!(fill(&work)["status"], "done");
    assert_eq!(fill(&work)["total"], 0);
    assert_eq!(served.ranged_reads(), 0, "nothing was read on its behalf");
}

// EP-2: the folder a fill is asked for is held to the same shape every other
// path on these routes is, and refused before anything is armed.
#[tokio::test]
async fn a_fill_of_something_that_is_not_a_folder_is_refused() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.post("/api/fill?path=albums/../etc").await).await;
    assert_eq!(status, 400);
    assert_eq!(refusal["error"], "bad_path");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        without_server(&work),
        json!({
            "library": "unlocked",
            "catalog": { "state": "caught_up", "stopped": null },
            "fill": null,
            "sync": null,
            "reconnect": null,
            "freeze": null,
            "delete": null,
        })
    );
}

/// The one finding a fill carries, as the work answer says it.
fn degraded_keyring() -> serde_json::Value {
    json!([{
        "path": null,
        "message": "the Library's Keyring is degraded: some of its replicas are missing or \
                    unreadable. Files still open, and the next run that writes to the Library \
                    repairs it",
        "reason": "keyring_degraded",
    }])
}

// KL-15: replica loss is never silent, and the person it matters most for is
// one who only reads. Every Entry a fill fetches reads the committed Keyring
// afresh and steps over the same lost replica, and the run says so once — as
// a finding beside its counts, which neither stops it nor declines anything:
// the files still open (spec: RV-2).
#[tokio::test]
async fn a_fill_whose_fetches_step_over_a_lost_keyring_replica_says_so_once() {
    let served = Served::library().await;
    served.degrade_the_keyring().await;

    assert_eq!(
        served.post("/api/fill?path=albums/2026").await.status(),
        202
    );
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let fill = fill(&work);
    assert_eq!(fill["status"], "done", "a degraded Keyring stops nothing");
    assert_eq!(fill["stopped"], serde_json::Value::Null);
    assert_eq!(
        (fill["done"].as_u64(), fill["total"].as_u64()),
        (Some(2), Some(2)),
        "both Entries were read through the replica after the lost one: {work}",
    );
    assert_eq!(declined(fill), Vec::<(String, String)>::new());
    assert_eq!(
        fill["findings"],
        degraded_keyring(),
        "once for the run, although both of its fetches met it: {work}",
    );
}

// The fetch behind `GET /api/file` placed the one Entry it was asked for, so
// the fill it arms never fetches that Entry and never reads what that fetch
// read. `books` holds nothing else, so the fill has nothing of its own to fetch
// at all — and what the opened file's fetch stepped over is still what its line
// says, because it is the only line a person who opened that file reads.
#[tokio::test]
async fn opening_a_file_behind_a_degraded_keyring_says_so_on_the_fill_it_arms() {
    let served = Served::library().await;
    served.degrade_the_keyring().await;

    let answer = served.get("/api/file?path=books/page-001.png").await;
    assert_eq!(answer.status(), 200, "the file still opens (spec: RV-2)");
    served.fill_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let fill = fill(&work);
    assert_eq!(fill["folder"], "books");
    assert_eq!(fill["status"], "done");
    assert_eq!(
        fill["total"], 0,
        "nothing left in the folder to fetch: {work}"
    );
    assert_eq!(fill["findings"], degraded_keyring(), "{work}");
}
