//! The upload, and the sync an ordinary drop arms: what lands, what a drop is
//! refused whole or part by part for, and the budgets one drop is taken within.

use coffret_logging::testing::CapturedLogs;
use coffret_server::Allowance;
use serde_json::{json, Value};
use tracing::Level;

use crate::support::{bytes, json as body_of, refusal_of, states, sync, written, Served};

// The whole of what a drop is for. Two files land in the folder, the listing
// shows them at once — nothing has committed them, so no catalog row exists and
// the folder itself is what knows they are there — and the sync the drop armed
// turns them into Entries this device has.
//
// Storage is away for the first half, which is what makes the two halves
// separable at all: the sync a drop arms would otherwise have finished before
// anything could look.
#[tokio::test]
async fn a_dropped_file_is_listed_at_once_and_becomes_an_entry_when_the_sync_lands() {
    let served = Served::library().await;
    served.halt_storage();

    let (status, answer) = body_of(
        served
            .upload(
                "albums/2026",
                &[("held.jpg", b"held"), ("moor.jpg", b"moor")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        written(&answer),
        ["albums/2026/held.jpg", "albums/2026/moor.jpg"]
    );
    assert_eq!(answer["refused"], json!([]));
    assert!(
        served.holds("albums/2026/held.jpg"),
        "the file is in the folder"
    );

    served.sync_idle().await;
    let (_, listing) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert_eq!(
        states(&listing),
        [
            ("held.jpg".to_owned(), "added".to_owned()),
            ("moor.jpg".to_owned(), "added".to_owned()),
            ("spring.jpg".to_owned(), "remote".to_owned()),
            ("summer.jpg".to_owned(), "remote".to_owned()),
        ],
        "a dropped file is a row of the folder before anything has committed it",
    );
    assert_eq!(
        listing["files"][0]["container"],
        serde_json::Value::Null,
        "nothing has been committed for it, so it lives in no Container yet",
    );

    // It is a real file in their own folder, so it opens like any other row.
    let opened = served.get("/api/file?path=albums/2026/held.jpg").await;
    assert_eq!(opened.status(), 200);
    assert_eq!(bytes(opened).await, b"held");

    served.resume_storage();
    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;

    let (_, listing) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert_eq!(
        states(&listing),
        [
            ("held.jpg".to_owned(), "present".to_owned()),
            ("moor.jpg".to_owned(), "present".to_owned()),
            ("spring.jpg".to_owned(), "remote".to_owned()),
            ("summer.jpg".to_owned(), "remote".to_owned()),
        ],
        "the sync carried them in, and they are ordinary Entries this device has",
    );
    assert_eq!(listing["files"][0]["container"], "one-file");
}

// PK-14, EP-10: a file this device had and no longer has is a finding, not a
// deletion, and it reaches the browser as the shape a page reads — the Entry,
// the sentence, and which finding it is in the two fields a declined fetch
// names one by, so that a page branching on it never has to read the prose.
#[tokio::test]
async fn a_finding_reaches_the_browser_with_its_reason_beside_the_sentence() {
    let served = Served::library().await;
    served.upload("albums", &[("gone.jpg", b"gone")]).await;
    served.sync_idle().await;

    std::fs::remove_file(served.local_path("albums/gone.jpg"))
        .expect("the synced file is on the disk to remove");
    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "done");
    assert_eq!(
        sync(&work)["findings"],
        json!([{
            "path": "albums/gone.jpg",
            "message": "this device had this file and it is gone; the Library still holds it",
            "reason": "surfaced",
            "surfaced": "DeletedLocally",
        }]),
    );
}

// A sync that Storage stopped is reported the way a fill that Storage stopped is
// — the state the retry is offered from — and the retry finishes once the store
// is back, with the files still sitting in the folder where the drop left them.
#[tokio::test]
async fn a_sync_storage_stopped_is_reported_and_finishes_when_the_store_comes_back() {
    let served = Served::library().await;
    served.halt_storage();

    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "stopped");
    assert_eq!(sync(&work)["stopped"]["error"], "storage");
    assert_eq!(sync(&work)["added"], 0);

    served.resume_storage();
    let (status, armed) = body_of(served.post("/api/sync").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        sync(&armed)["status"],
        "syncing",
        "the failure it is retrying is off the screen the moment the retry is armed",
    );

    served.sync_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "done");
    assert_eq!(sync(&work)["added"], 1);
    assert_eq!(sync(&work)["stopped"], serde_json::Value::Null);
    assert_eq!(sync(&work)["findings"], json!([]));
}

/// Asserts that `findings` is exactly the one finding a sync whose commit put a
/// Keyring replica back carries.
///
/// The generation is whatever the fixture's own commits left as the head, so
/// what is pinned is everything around it: one replica, rewritten, in the
/// words the command line says it in, and no path.
fn one_repair(findings: &Value) {
    let [finding] = findings.as_array().map(Vec::as_slice).unwrap_or_default() else {
        panic!("expected exactly the repair, got {findings}");
    };
    assert_eq!(finding["reason"], "keyring_repaired", "{finding}");
    assert_eq!(finding["path"], Value::Null, "{finding}");
    assert_eq!(finding["surfaced"], Value::Null, "{finding}");
    let message = finding["message"].as_str().expect("a sentence");
    let generation = message
        .strip_prefix("repaired the Keyring: 1 replica of generation ")
        .and_then(|rest| {
            rest.strip_suffix(" was missing or unreadable, and was rewritten from a surviving one")
        })
        .unwrap_or_else(|| panic!("not the repair's sentence: {message}"));
    assert!(
        generation.parse::<u64>().is_ok(),
        "the generation is a number: {message}"
    );
}

// KL-15: a repair performed is never silent. A sync the explorer started that
// found the committed Keyring short put it back before it committed, and the
// work answer says so beside the run that finished — which it does not stop,
// and which needs nobody to act on it.
#[tokio::test]
async fn a_sync_that_repaired_the_keyring_says_so_on_the_run_that_finished() {
    let served = Served::library().await;
    served.degrade_the_keyring().await;

    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        sync(&work)["status"],
        "done",
        "a repair stops nothing: {work}"
    );
    assert_eq!(sync(&work)["stopped"], Value::Null);
    assert_eq!(sync(&work)["added"], 1);
    one_repair(&sync(&work)["findings"]);
}

// And a sync whose commit failed after the repair says it too, on the run that
// stopped: the replica it put back stands on Storage whatever became of the
// batch. The run is stopped by what refused the commit and by nothing else.
#[tokio::test]
async fn a_sync_whose_commit_failed_after_a_repair_says_the_repair_on_the_stopped_run() {
    let served = Served::library().await;
    served.degrade_the_keyring().await;
    served.refuse_commits();

    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.sync_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(sync(&work)["status"], "stopped", "{work}");
    assert_eq!(sync(&work)["stopped"]["error"], "storage", "{work}");
    assert_eq!(sync(&work)["added"], 0);
    one_repair(&sync(&work)["findings"]);
}

// EP-9: a folder no mapping of this device reaches has nowhere to put a single
// one of the files, so the whole drop is refused at once rather than once per
// file — the same verdict the listing already shows over the rows.
#[tokio::test]
async fn a_drop_onto_a_folder_that_is_not_on_this_device_is_refused_whole() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.upload("books", &[("new.png", b"new")]).await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "refused_placement");
    assert_eq!(refusal["reason"], "unmapped");

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        work["sync"],
        serde_json::Value::Null,
        "nothing landed, so there is nothing to carry in",
    );
}

// EP-13: a mapped folder whose marker names another identity is not the folder
// the mapping was recorded against, and nothing is placed into it. The route is
// one of the single writers EP-11 names, so it fails that request as a whole
// when a placement it asked for is refused (EP-11, EP-13) — and every part of a
// drop onto a folder goes through that folder's one mapping (EP-9), so the
// refusal is no truer of the next part than of the first. The drop is refused as
// a whole, at the first part, rather than read to the end and answered with one
// refused entry per file.
#[tokio::test]
async fn a_drop_into_a_refused_root_is_refused_whole_rather_than_part_by_part() {
    let served = Served::library().await;

    // The folder in front of the device is somebody else's copy of the one that
    // was registered: the marker is a marker, and it names another identity.
    std::fs::write(served.local_path(".coffret/root"), "0011223344556677\n")
        .expect("the mapped root's marker can be rewritten");
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(
        served
            .upload("albums", &[("first.png", b"one"), ("second.png", b"two")])
            .await,
    )
    .await;
    // Two parts, one refusal — the point of the whole case. The answer is the
    // refusal itself and carries no `refused` array, because a refused root is
    // not something one of the files was refused for.
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "refused_placement");
    assert_eq!(refusal["reason"], "refused_root");
    assert_eq!(
        refusal["refused"],
        Value::Null,
        "the refusal is the answer, not an entry in one: {refusal}",
    );

    // Answered in the middle of a request the browser is still sending, which it
    // may read as a transfer that failed rather than as an answer — so the log
    // is the half of it that arrives either way, exactly as it is for a budget
    // this route stops a drop at. It also says which of EP-13's cases this was,
    // which the sentence deliberately does not.
    assert_eq!(
        refusal_of(&logs, "answer"),
        "Device::RootRefused: MarkerMismatch",
    );

    // And it is the same sentence a fetch into that root is refused with: one
    // state of the mapping, said one way, whichever flow met it.
    let (_, fetched) = body_of(served.get("/api/file?path=albums/notes.txt").await).await;
    assert_eq!(fetched["reason"], "refused_root");
    assert_eq!(refusal["message"], fetched["message"]);

    assert!(
        !served.holds("albums/first.png"),
        "nothing is placed into a folder that will not vouch for itself",
    );
    assert!(!served.holds("albums/second.png"));

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        work["sync"],
        Value::Null,
        "nothing landed, so there is nothing to carry in",
    );
    assert_eq!(work["freeze"], Value::Null);
}

// EP-11, EP-13: a marker the operating system will not let this process read
// decides nothing about which folder the mapped root is — a permission is not a
// mismatch, and nobody may be sent to record the mapping again over one. It is
// a fact about the root every part of the drop goes through all the same, and
// one decided before the first part was read: reported as one file's business
// the drop would read the next part, meet it again, and answer with one refused
// entry per file for a condition none of them caused. So the request stops at
// the first part, and the sentence a person gets says nothing about a mapping.
#[cfg(unix)]
#[tokio::test]
async fn a_root_the_system_would_not_answer_about_stops_the_drop() {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    let served = Served::library().await;

    // Arranged after the fixture has filled, so what the case is about is the
    // drop and not a start-up that could not read the root either.
    let marker = served.local_path(".coffret/root");
    std::fs::set_permissions(&marker, Permissions::from_mode(0o000))
        .expect("the mapped root's marker can be made unreadable");
    assert!(
        std::fs::read(&marker).is_err(),
        "this case needs a process the marker's own mode keeps out, and this one is not kept \
         out — a run as root cannot arrange what it is about",
    );
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(
        served
            .upload("albums", &[("first.png", b"one"), ("second.png", b"two")])
            .await,
    )
    .await;

    // Two parts, one refusal, and it is the answer rather than an entry in one.
    assert_eq!(status, 500);
    assert_eq!(refusal["error"], "server");
    assert_eq!(
        refusal["refused"],
        Value::Null,
        "the refusal is the answer, not something one of the files was refused for: {refusal}",
    );
    assert_eq!(
        refusal["reason"],
        Value::Null,
        "nothing was declined about a mapping, because nothing about it was read: {refusal}",
    );
    assert!(
        !refusal["message"]
            .as_str()
            .expect("a refusal carries a sentence")
            .contains("record"),
        "a permission is not a mismatch, and nobody is sent to record the mapping again over \
         one: {refusal}",
    );
    // And the folder itself is in neither half of the answer: the page is told
    // what every other failure of this machine's own tells it, and what could
    // not be read is a local path an event may not carry either (spec: EL-1).
    let root = marker
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the marker stands inside the management area of the mapped root")
        .display()
        .to_string();
    assert!(
        !refusal.to_string().contains(&root),
        "the folder the disk would not answer about is nobody's to read here: {refusal}",
    );
    assert!(
        !logs.text().contains(&root),
        "nor the log's:\n{}",
        logs.text(),
    );

    // The log is where the operation and the kind survive, and it says the
    // marker went unread rather than that a mapping is wrong — the two are told
    // apart by the identity, which is what a reader counting these groups by.
    assert_eq!(
        refusal_of(&logs, "answer"),
        "Device::RootUnvouched: Local::Io(operation=read, kind=PermissionDenied)",
    );
    assert!(
        logs.at(Level::ERROR)
            .into_iter()
            .all(|event| event.field("operation") != "upload"),
        "no part was refused on its own account:\n{}",
        logs.text(),
    );

    assert!(
        !served.holds("albums/first.png"),
        "nothing is placed into a root this device could not ask about",
    );
    assert!(!served.holds("albums/second.png"));
    assert_eq!(
        served
            .folder_names("albums")
            .into_iter()
            .filter(|name| name.starts_with(".coffret-fetch-"))
            .count(),
        0,
        "and the part it stopped at leaves no scratch behind either",
    );

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        work["sync"],
        Value::Null,
        "nothing landed, so there is nothing to carry in",
    );

    // Given back so the fixture's temporary folder can be cleaned up after it.
    std::fs::set_permissions(&marker, Permissions::from_mode(0o600))
        .expect("the marker's mode can be given back");
}

// PK-10, PK-12: coffret cannot replace an Entry inside a Pack yet, and writing
// the file anyway would leave it in the folder with no sync able to carry it in.
// It is refused by name, and the file beside it lands.
#[tokio::test]
async fn a_part_the_library_holds_inside_a_pack_is_refused_and_its_sibling_lands() {
    let served = Served::packed_library().await;
    served.halt_storage();

    let (status, answer) = body_of(
        served
            .upload(
                "books",
                &[("page-001.png", b"mine"), ("page-002.png", b"next")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer), ["books/page-002.png"]);
    assert_eq!(
        answer["refused"],
        json!([{
            "name": "page-001.png",
            "error": "refused_placement",
            "reason": "pack_resident",
            "message": "the Library holds this file inside a Pack, and coffret cannot replace \
                        one of those yet",
        }]),
    );
    assert!(
        !served.holds("books/page-001.png"),
        "the refusal was decided before any byte was written",
    );
    assert!(served.holds("books/page-002.png"));

    // The sibling landed, so a sync was armed: it is waited out here rather than
    // left running past the end of the case, over folders the case is about to
    // remove.
    served.sync_idle().await;
}

// EP-2: a part's own name is held to the same shape every other path on these
// routes is, and a part that climbed out of the folder it was dropped on is
// refused by the name it was sent under — there being no Entry Path to report it
// as.
#[tokio::test]
async fn a_part_whose_name_is_not_an_entry_path_is_refused_by_name() {
    let served = Served::library().await;
    served.halt_storage();

    let (status, answer) = body_of(
        served
            .upload("albums", &[("../escaped.jpg", b"out"), ("kept.jpg", b"in")])
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer), ["albums/kept.jpg"]);
    assert_eq!(answer["refused"][0]["name"], "../escaped.jpg");
    assert_eq!(answer["refused"][0]["error"], "bad_path");
    assert!(served.holds("albums/kept.jpg"));
    served.sync_idle().await;
}

// EP-11: an upload that stopped half way leaves a name under the prefix a scan
// steps over, so nothing reads it as a file somebody added — a listing least of
// all, which is the one place it would look like one.
#[tokio::test]
async fn the_scratch_of_an_interrupted_upload_is_not_a_row() {
    let served = Served::library().await;
    served.plant_locally("albums/.coffret-fetch-incoming-abcd.part", b"half a file");

    let (_, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(
        states(&listing),
        [
            ("caf\u{e9}.jpg".to_owned(), "remote".to_owned()),
            ("cover.png".to_owned(), "remote".to_owned()),
            ("notes.txt".to_owned(), "remote".to_owned()),
        ],
        "coffret's own scratch is not a file anybody put there",
    );

    let (status, refusal) = body_of(
        served
            .get("/api/file?path=albums/.coffret-fetch-incoming-abcd.part")
            .await,
    )
    .await;
    assert_eq!(status, 404, "and it is not something to be read either");
    assert_eq!(refusal["error"], "no_such_entry");
}

// The three budgets one drop is taken within (spec: LA-9), and what passing one
// of them does (spec: LA-10). First the whole request: the route is mounted with
// a ceiling on it, and a request that passes it stops there rather than being
// read to the end and refused afterwards. What it leaves is nothing at all: no
// file under a final name, and no scratch either — the scratch name the bytes
// were going to goes with the incoming file that was dropped (spec: EP-11).
#[tokio::test]
async fn a_drop_past_the_request_budget_is_stopped_and_leaves_nothing() {
    let served = Served::within(Allowance {
        request_bytes: 64,
        ..Allowance::generous()
    })
    .await;

    let (status, refusal) = body_of(
        served
            .upload("albums", &[("page-001.jpg", &[b'x'; 4096])])
            .await,
    )
    .await;
    assert_eq!(status, 413);
    assert_eq!(refusal["error"], "bad_request");
    assert_eq!(
        served.folder_names("albums"),
        Vec::<String>::new(),
        "a drop that was refused leaves the folder as it found it, scratch included",
    );
    assert_eq!(
        refusal["written"],
        json!([]),
        "and the answer says so, rather than leaving the page to guess",
    );
}

// The same for one part of it. A page of a scanned book has a size past which it
// is not a page, and a part that passes it takes the request with it rather than
// being one refusal among the answer's — there is no reason to read the rest of a
// request already known to be more than this route takes.
#[tokio::test]
async fn a_part_past_the_part_budget_is_stopped_and_leaves_nothing() {
    let served = Served::within(Allowance {
        part_bytes: 8,
        ..Allowance::generous()
    })
    .await;
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(
        served
            .upload("albums", &[("page-001.jpg", b"more than eight bytes")])
            .await,
    )
    .await;
    assert_eq!(status, 413);
    assert_eq!(refusal["error"], "bad_request");
    assert_eq!(
        served.folder_names("albums"),
        Vec::<String>::new(),
        "the part it stopped at was never finished, so nothing of it is there",
    );

    // Answered in the middle of a request the caller is still sending, which a
    // browser may read as a transfer that failed rather than as an answer — so
    // the log is the half of it that arrives either way.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "upload");
}

// And it says which file it stopped at. A person who dropped three hundred scans
// and is told one of them is over the limit on its own has nothing to act on
// until they know which one, and a person-facing refusal may name a file that
// person owns (spec: EL-1). The event beside it is the other half of that same
// rule: a diagnostic record says what was refused and never what it was called,
// so the two renderings differ by exactly the name.
#[tokio::test]
async fn a_part_past_the_part_budget_says_which_file_was_over_it() {
    let served = Served::within(Allowance {
        part_bytes: 8,
        ..Allowance::generous()
    })
    .await;
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(
        served
            .upload("albums", &[("page-001.jpg", b"more than eight bytes")])
            .await,
    )
    .await;
    assert_eq!(status, 413);
    let said = refusal["message"]
        .as_str()
        .expect("a refusal carries one sentence");
    assert!(
        said.contains("page-001.jpg"),
        "the one file that was over the budget is named: {said}",
    );

    // And the name is in neither the event's fields nor its message (spec: EL-1).
    logs.assert_free_of(&["page-001.jpg"]);
    let event = logs.only(Level::WARN);
    // The other half of the same rule, which an absence cannot show on its own:
    // the event still says what was refused, in the same words with the name
    // left out. Without this the case would pass just as well against an event
    // that had stopped saying which budget the drop passed — a name kept out of
    // a record that says nothing is not the boundary being held.
    assert_eq!(
        event.field("defect"),
        "one file in it is over that on its own, so dropping fewer beside it changes nothing",
    );
}

// And for how many parts there are. Here the drop had already landed two files
// before it passed the budget, and they stay: what EP-11 promises is that no half
// file appears under a final name, not that a refused request unwinds. The one it
// stopped at is not there under any name.
#[tokio::test]
async fn a_drop_of_more_parts_than_one_gesture_carries_is_stopped() {
    let served = Served::within(Allowance {
        parts: 2,
        ..Allowance::generous()
    })
    .await;

    let (status, refusal) = body_of(
        served
            .upload(
                "albums",
                &[
                    ("one.jpg", b"one"),
                    ("two.jpg", b"two"),
                    ("three.jpg", b"three"),
                ],
            )
            .await,
    )
    .await;
    assert_eq!(status, 413);
    assert_eq!(refusal["error"], "bad_request");
    assert_eq!(
        served.folder_names("albums"),
        ["one.jpg", "two.jpg"],
        "what landed is whole and stays; the part it stopped at left nothing, \
         scratch included",
    );
    // And the refusal names them, for the page to show.
    assert_eq!(
        refusal["written"],
        json!(["albums/one.jpg", "albums/two.jpg"]),
    );
}

// The room question beside the budgets (spec: LA-11): a drop far larger than the
// disk it is aimed at is refused before it fills it. The volume's answer is
// fabricated, because a disk with nothing left on it is not a thing a case may
// arrange — and the number is what this is about, not where it came from.
#[tokio::test]
async fn a_drop_this_device_has_no_room_for_is_refused_before_it_is_written() {
    let served = Served::within(Allowance {
        space: |_| Ok(64),
        ..Allowance::generous()
    })
    .await;
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(
        served
            .upload("albums", &[("page-001.jpg", &[b'x'; 4096])])
            .await,
    )
    .await;
    assert_eq!(status, 507);
    assert_eq!(
        refusal["error"], "server",
        "it is this machine's state rather than anything the browser did",
    );
    assert_eq!(
        served.folder_names("albums"),
        Vec::<String>::new(),
        "and it is said before the bytes are written rather than after",
    );
    assert_eq!(refusal["written"], json!([]));

    // The sentence says nothing about how full the disk is, on purpose, and this
    // refusal carries no failure underneath it for the answer to record — so
    // without this line a device refusing every drop for want of room would say
    // so nowhere anybody keeping it could find it.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "upload");
    assert_eq!(event.field("available"), "64");
}

// The other half of the room question (spec: LA-11): what is asked for when the
// request declared nothing. A caller that streams its body instead of sending a
// `FormData` says no `Content-Length`, and is not refused for that — the fence
// asks for one part's ceiling (spec: LA-9) instead of for what the request said
// it was bringing. So a drop of one kilobyte meets a gibibyte-sized question,
// and a volume with less than that free refuses it.
//
// That is a real thing a person meets rather than a corner: it is the whole of
// the difference between a body gathered before it is sent and one streamed as
// it is produced — a `fetch` handed a `ReadableStream`, or any client sending
// chunked — aimed at the same route, and the number the fence weighs is a
// million times the drop.
#[tokio::test]
async fn an_upload_that_declares_no_length_is_weighed_against_one_parts_ceiling() {
    // Room enough for the drop many times over, and short of one part's
    // ceiling — so only a fence asking for the ceiling refuses this.
    let allowance = Allowance {
        space: |_| Ok(64 * 1024 * 1024),
        ..Allowance::generous()
    };
    let served = Served::within(allowance).await;
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(
        served
            .upload_undeclared("albums", &[("page-001.jpg", &[b'x'; 1024])])
            .await,
    )
    .await;
    assert_eq!(status, 507);
    assert_eq!(
        refusal["error"], "server",
        "it is this machine's state rather than anything the caller did, and \
         declaring no length is not itself a refusal",
    );
    assert_eq!(
        served.folder_names("albums"),
        Vec::<String>::new(),
        "and it is said before the bytes are written rather than after",
    );

    // Which number was weighed is the whole case: one part's ceiling, and not
    // the kilobyte that was actually coming.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "upload");
    assert_eq!(event.number("coming"), allowance.part_bytes as i64);
    assert_eq!(event.field("available"), "67108864");
}
