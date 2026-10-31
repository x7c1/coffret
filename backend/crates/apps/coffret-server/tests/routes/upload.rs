//! The upload, and the sync an ordinary drop arms: what lands, what a drop is
//! refused whole or part by part for, and the budgets one drop is taken within.

use coffret_logging::testing::CapturedLogs;
use coffret_server::Allowance;
use serde_json::{json, Value};
use tracing::Level;

use crate::support::{
    bytes, freeze, json as body_of, refusal_of, rows_of, states, sync, written, Served,
};

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

// EP-11, FM-9: a dropped file keeps the time the explorer sent with it. The
// part's field name is the file's own modification time in milliseconds, the
// file is stamped with it before it appears, and the Entry the sync makes of it
// carries the second it falls in — the earlier one before 1970. Its birth time
// is none at all: what its filesystem reports is when the drop wrote it, which
// is not the person's file's.
#[tokio::test]
async fn a_dropped_file_keeps_the_time_it_was_sent_with_and_records_no_birth_time() {
    let served = Served::library().await;

    let (status, answer) = body_of(
        served
            .upload_modified(
                "albums",
                &[
                    ("scanned.jpg", b"scanned in 2015", 1_444_000_000_999),
                    ("older.jpg", b"older than the epoch", -1_500),
                ],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer), ["albums/scanned.jpg", "albums/older.jpg"]);
    served.sync_idle().await;

    let scanned = served.entry("albums/scanned.jpg").await;
    assert_eq!(scanned.mtime.as_unix_seconds(), 1_444_000_000);
    assert_eq!(scanned.btime, None);
    let older = served.entry("albums/older.jpg").await;
    assert_eq!(older.mtime.as_unix_seconds(), -2);
    assert_eq!(older.btime, None);

    let (_, listing) = body_of(served.get("/api/list?path=albums").await).await;
    let shown = listing["files"]
        .as_array()
        .expect("the listing has files")
        .iter()
        .find(|row| row["name"] == "scanned.jpg")
        .expect("the dropped file is a row");
    assert_eq!(shown["mtime"], "2015-10-04T23:06:40Z");
}

// EP-11, FM-9: a part that says no time — the field name a caller that knows
// nothing of times sends — is stored as it always was, with the time it was
// written at, and still with no birth time, since it was dropped all the same.
// A file put into the folder any other way keeps the birth time its filesystem
// reports, where it reports one.
#[tokio::test]
async fn a_part_without_a_time_keeps_the_written_one_and_a_copied_file_keeps_its_birth() {
    let served = Served::library().await;
    served.plant_locally("albums/copied.jpg", b"copied by hand");

    let (status, _) = body_of(served.upload("albums", &[("plain.jpg", b"plain")]).await).await;
    assert_eq!(status, 200);
    served.sync_idle().await;

    let on_disk = std::fs::metadata(served.local_path("albums/plain.jpg"))
        .expect("the dropped file is in the folder");
    let plain = served.entry("albums/plain.jpg").await;
    assert_eq!(
        Some(plain.mtime.as_unix_seconds()),
        seconds(on_disk.modified())
    );
    assert_eq!(plain.btime, None);

    let on_disk = std::fs::metadata(served.local_path("albums/copied.jpg"))
        .expect("the copied file is in the folder");
    let copied = served.entry("albums/copied.jpg").await;
    assert_eq!(
        copied.btime.map(|btime| btime.as_unix_seconds()),
        seconds(on_disk.created()),
        "a file the drop did not place keeps whatever birth time its filesystem reports",
    );
}

/// A time the filesystem reported, as whole seconds after the epoch, or `None`
/// where it reported none.
///
/// Every time these cases read is after the epoch, so the seconds are the ones
/// a scan reads.
fn seconds(reported: std::io::Result<std::time::SystemTime>) -> Option<i64> {
    let at = reported.ok()?;
    let after = at
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a file written now is after the epoch");
    Some(i64::try_from(after.as_secs()).expect("a time written now fits"))
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

// EP-9: a drop onto the Library root goes through the Library-root mapping
// alone. A top-level mapping stands for its own subtree and not for what sits
// beside it, so a device that maps only `albums` refuses the whole drop — even
// the part `albums/new.png`, whose Entry Path falls under that mapping.
#[tokio::test]
async fn a_drop_onto_the_library_root_without_a_root_mapping_is_refused_whole() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(
        served
            .upload("", &[("albums/new.png", b"new"), ("notes.txt", b"n")])
            .await,
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "refused_placement");
    assert_eq!(refusal["reason"], "unmapped");
    assert!(
        !served.local_path("new.png").exists(),
        "nothing landed under the mapping the part's name begins with",
    );

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
// when a placement it asked for is refused (spec: EP-11, EP-13) — and every part
// of a drop onto a folder goes through that folder's one mapping (spec: EP-9),
// so the refusal is no truer of the next part than of the first. The drop is
// refused as a whole, at the first part, rather than read to the end and
// answered with one refused entry per file.
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

// EP-16: the folder a desktop keeps its trash in directly under the mapped root
// is no place to drop into, and the part is refused in its own words — the
// folder is the desktop's trash, not a name coffret keeps — while a sibling
// elsewhere in the same drop lands.
#[tokio::test]
async fn a_part_dropped_into_the_desktop_trash_is_refused_in_its_own_words() {
    let served = Served::library().await;
    served.halt_storage();

    let (status, answer) = body_of(
        served
            .upload(
                "",
                &[(".Trash/x.jpg", b"binned"), ("albums/kept.jpg", b"in")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer), ["albums/kept.jpg"]);
    let refused = &answer["refused"][0];
    assert_eq!(refused["name"], ".Trash/x.jpg");
    assert_eq!(refused["error"], "declined");
    assert_eq!(refused["reason"], "trash_folder");
    let said = refused["message"]
        .as_str()
        .expect("a refusal carries a sentence");
    assert!(said.contains("trash"), "{said}");
    assert!(
        !said.contains(".coffret"),
        "the trash is not one of coffret's own names: {said}",
    );
    assert!(
        !served.holds(".Trash/x.jpg"),
        "nothing was written into the trash",
    );
    assert!(served.holds("albums/kept.jpg"));
    served.sync_idle().await;
}

// EP-17: a name the operating system keeps is filtered out of a browser's drop
// on the page, and a part carrying one that arrives anyway is refused in its
// own words — at any depth, and a volume's own folder directly under the mapped
// root — while a sibling in the same drop lands.
#[tokio::test]
async fn a_part_carrying_a_name_the_operating_system_keeps_is_refused_in_its_own_words() {
    let served = Served::library().await;
    served.halt_storage();

    let (status, answer) = body_of(
        served
            .upload(
                "",
                &[
                    ("albums/.DS_Store", b"finder"),
                    ("albums/__MACOSX/._kept.jpg", b"apple double"),
                    (".Spotlight-V100/index", b"spotlight"),
                    ("albums/kept.jpg", b"in"),
                ],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer), ["albums/kept.jpg"]);
    let refused = answer["refused"]
        .as_array()
        .expect("the refusals are listed");
    let names: Vec<&str> = refused
        .iter()
        .map(|one| one["name"].as_str().expect("a refusal names its part"))
        .collect();
    assert_eq!(
        names,
        [
            "albums/.DS_Store",
            "albums/__MACOSX/._kept.jpg",
            ".Spotlight-V100/index"
        ]
    );
    for one in refused {
        assert_eq!(one["error"], "declined");
        assert_eq!(one["reason"], "system_file");
        let said = one["message"]
            .as_str()
            .expect("a refusal carries a sentence");
        assert!(said.contains("operating system"), "{said}");
    }
    assert!(!served.holds("albums/.DS_Store"));
    assert!(served.holds("albums/kept.jpg"));
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
async fn a_drop_of_more_parts_than_one_request_carries_is_stopped() {
    let served = Served::within(Allowance {
        parts: 2,
        ..Allowance::generous()
    })
    .await;
    let logs = CapturedLogs::capture();

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
    // Recorded as the request it refused: an upload, not an import.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "upload");
}

// The numbers the binary serves within (spec: LA-9). The count is 65,536, so a
// drop of several volumes, or of a photo library, is one request, while the byte
// budgets that guard the loopback socket stay where they were: 64 GiB in all and
// 1 GiB a part.
#[test]
fn the_binary_takes_a_request_of_65536_parts_within_the_same_byte_budgets() {
    let served = Allowance::generous();
    assert_eq!(served.parts, 65_536);
    assert_eq!(served.request_bytes, 64 * 1024 * 1024 * 1024);
    assert_eq!(served.part_bytes, 1024 * 1024 * 1024);
}

// And the count is met where the binary's numbers put it: a request of exactly
// 65,536 parts is taken, and one of 65,537 is stopped. The parts past the one
// file name no file, which the count weighs as it weighs a file (spec: LA-9), so
// the case reaches the budget without landing tens of thousands of files.
#[tokio::test]
async fn a_drop_of_65536_parts_is_taken_and_one_more_is_stopped() {
    let served = Served::within(Allowance::generous()).await;
    let most = Allowance::generous().parts;

    let (status, answer) = body_of(
        served
            .upload_padded("albums", &[("one.jpg", b"one")], most - 1)
            .await,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(written(&answer), ["albums/one.jpg"]);

    let (status, refusal) = body_of(
        served
            .upload_padded("albums", &[("two.jpg", b"two")], most)
            .await,
    )
    .await;
    assert_eq!(status, 413, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");
    assert_eq!(
        refusal["written"],
        json!(["albums/two.jpg"]),
        "the file before the budget landed, and the request stopped at the part past it",
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

/// The pages of the book the cases about a cut-off drop bring in.
const BOOK: [(&str, &[u8]); 3] = [
    ("page-001.jpg", b"the first page"),
    ("page-002.jpg", b"the second page"),
    ("page-003.jpg", b"the third page"),
];

/// Lands the first two pages of [`BOOK`] in `scans/vol-1` as a drop that asked
/// for a Pack, and stops its server while it is still receiving: a server
/// started afterwards, and started up, stands in place of `served`.
async fn cut_off(served: &mut Served) {
    let drop = served.upload_book_streamed("scans/vol-1");
    for (name, content) in &BOOK[..2] {
        served.until_landed(&drop.send(name, content)).await;
    }
    drop.abandon();
    served.restart();
    served.start_up().await;
    served.freeze_idle().await;
}

/// Every page of [`BOOK`] is an Entry this device has in `scans/vol-1`,
/// packed by one run into one Pack, and nothing waits for a Pack any more.
async fn one_pack_of_the_book(served: &Served) {
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["folder"], "scans/vol-1");
    assert_eq!(freeze(&work)["status"], "done", "{work}");
    assert_eq!(freeze(&work)["packs"], 1, "one book, one Pack: {work}");
    assert_eq!(freeze(&work)["entries"], BOOK.len());
    assert_eq!(
        rows_of(served, "scans/vol-1").await,
        BOOK.map(|(name, _)| (name.to_owned(), "present".to_owned(), "pack".to_owned())),
    );
    assert_eq!(
        served.reservations().await,
        0,
        "the one reservation went with the commit"
    );
}

// PK-17, LA-12: the same folder dropped again after a drop of it was cut off
// by its server stopping takes the cut-off drop over rather than reserving
// beside it. The pages the first drop received are not refused as inside a
// Pack — none was packed — a page sent again replaces the one on disk, and the
// freeze that follows packs every page of both drops as one Pack.
#[tokio::test]
async fn a_cut_off_drop_dropped_again_is_taken_over_and_packed_as_one() {
    let mut served = Served::library().await;
    cut_off(&mut served).await;

    let logs = CapturedLogs::capture();
    let mut again = BOOK;
    again[0].1 = b"the first page, scanned again";
    let (status, answer) = body_of(served.upload_book("scans/vol-1", &again).await).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        written(&answer).len(),
        BOOK.len(),
        "every page lands, the ones received before included: {answer}"
    );
    assert_eq!(
        answer["refused"],
        json!([]),
        "no page is refused as inside a Pack"
    );
    assert!(
        logs.text()
            .contains("a drop cut off part way was taken over by a drop of its folder"),
        "{}",
        logs.text()
    );
    served.freeze_idle().await;

    one_pack_of_the_book(&served).await;
    assert_eq!(
        bytes(served.get("/api/file?path=scans/vol-1/page-001.jpg").await).await,
        b"the first page, scanned again",
        "the page sent again replaced the one received before",
    );
}

// PK-17, LA-12: the drop that takes a cut-off one over starts from the pages
// that drop received — they are in the reservation before this drop sends a
// page — so a drop of only the rest of the book still packs all of it.
#[tokio::test]
async fn a_drop_taking_a_cut_off_one_over_starts_from_the_pages_it_received() {
    let mut served = Served::library().await;
    cut_off(&mut served).await;

    let rest = served.upload_book_streamed("scans/vol-1");
    let (name, content) = BOOK[2];
    served.until_landed(&rest.send(name, content)).await;
    assert_eq!(
        served.reserved_paths().await,
        [BOOK.map(|(name, _)| format!("scans/vol-1/{name}")).to_vec()],
        "one reservation, naming the pages received before and the one landed now",
    );
    let (status, answer) = body_of(rest.finish().await).await;
    assert_eq!(status, 200, "{answer}");
    served.freeze_idle().await;

    one_pack_of_the_book(&served).await;
}

// PK-17: a drop that took a cut-off one over and is then stopped as a whole —
// here a page past the part budget — does not release it: the reservation
// stays a drop in progress, naming what both drops landed, and the next drop of
// the folder still brings the whole book in as one Pack. Released, the pages
// the first drop received would be left loose for a sync to carry in one
// Container each.
#[tokio::test]
async fn a_drop_taking_a_cut_off_one_over_that_is_stopped_leaves_it_in_progress() {
    let mut served = Served::within(Allowance {
        part_bytes: 64,
        ..Allowance::generous()
    })
    .await;
    cut_off(&mut served).await;

    let (status, refusal) = body_of(
        served
            .upload_book(
                "scans/vol-1",
                &[BOOK[2], ("page-004.jpg", &[b'x'; 128][..])],
            )
            .await,
    )
    .await;
    assert_eq!(status, 413, "{refusal}");
    assert_eq!(refusal["written"], json!(["scans/vol-1/page-003.jpg"]));
    served.freeze_idle().await;
    assert_eq!(
        served.reserved_paths().await,
        [BOOK.map(|(name, _)| format!("scans/vol-1/{name}")).to_vec()],
        "the one reservation is kept, naming the pages of both drops",
    );

    let (status, answer) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200, "{answer}");
    served.freeze_idle().await;
    one_pack_of_the_book(&served).await;
}

// PK-17: a drop that took a cut-off one over and every file of which is
// refused still finishes the book the cut-off drop received: its freeze packs
// those pages as one Pack and releases the reservation.
#[tokio::test]
async fn a_drop_taking_a_cut_off_one_over_with_every_file_refused_packs_what_it_received() {
    let mut served = Served::library().await;
    cut_off(&mut served).await;

    let (status, answer) = body_of(
        served
            .upload_book("scans/vol-1", &[("../escaped.jpg", b"out")])
            .await,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(written(&answer), Vec::<String>::new());
    assert_eq!(answer["refused"][0]["error"], "bad_path");
    served.freeze_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(freeze(&work)["status"], "done", "{work}");
    assert_eq!(freeze(&work)["packs"], 1, "{work}");
    assert_eq!(
        freeze(&work)["entries"],
        2,
        "the pages the cut-off drop received"
    );
    assert_eq!(
        rows_of(&served, "scans/vol-1").await,
        BOOK[..2]
            .iter()
            .map(|(name, _)| ((*name).to_owned(), "present".to_owned(), "pack".to_owned()))
            .collect::<Vec<_>>(),
    );
    assert_eq!(served.reservations().await, 0);
}
