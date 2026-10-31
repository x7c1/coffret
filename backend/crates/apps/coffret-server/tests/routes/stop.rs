//! Stopping an add, a freeze and a fill from the Activity panel before it
//! commits (spec: LA-34, LA-35, LA-36).
//!
//! A stop is a request: the route marks the run asked and answers at once,
//! and the run answers by ending. Each case holds the run where it is meant to
//! be stopped — receiving, waiting its turn, inside Storage, inside its commit
//! — by the fixture's own switches rather than by sleeping, and then reads
//! what is on this device and in the Library afterwards.

use serde_json::{json, Value};

use crate::support::{fill, freeze, json as body_of, rows_of, sync, written, Served};

/// Asks for a stop, and answers with the work answer it is met with.
async fn stop(served: &Served, body: Value) -> Value {
    let (status, work) = body_of(served.post_json("/api/stop", &body).await).await;
    assert_eq!(status, 202, "{work}");
    work
}

/// The entries of the log, newest first, as `(kind, entry)`.
fn logged(work: &Value) -> Vec<(String, Value)> {
    work["log"]
        .as_array()
        .expect("the work answer carries the log")
        .iter()
        .map(|entry| {
            (
                entry["kind"]
                    .as_str()
                    .expect("an entry has a kind")
                    .to_owned(),
                entry.clone(),
            )
        })
        .collect()
}

/// The one stopped add the log holds.
fn stopped_add(work: &Value) -> Value {
    let adds: Vec<Value> = logged(work)
        .into_iter()
        .filter(|(kind, _)| kind == "add")
        .map(|(_, entry)| entry["add"].clone())
        .collect();
    assert_eq!(adds.len(), 1, "one add was stopped: {work}");
    adds[0].clone()
}

/// The names of the rows of `folder` in the Library's listing.
async fn names_in(served: &Served, folder: &str) -> Vec<String> {
    rows_of(served, folder)
        .await
        .into_iter()
        .map(|(name, _, _)| name)
        .collect()
}

/// Waits until a run is inside Storage, held there.
async fn inside_storage(served: &Served) {
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
}

/// The work answer, once more.
async fn work_of(served: &Served) -> Value {
    body_of(served.get("/api/work").await).await.1
}

// LA-36, PK-17: a Pack drop stopped while its pages are still arriving is answered
// stopped and undone at once: the page that landed is removed, the folder the
// drop made goes with it, its reservation is released, the file that was in
// the folder before stays, and nothing reaches the Library.
#[tokio::test]
async fn a_pack_drop_stopped_while_receiving_leaves_nothing_behind() {
    let served = Served::library().await;
    served.plant_locally("books/before.png", b"the person's own");

    let drop = served.add_streamed("books/vol-1", "drop-1", true);
    let landed = drop.send("page-001.png", b"page one");
    served.until_landed(&landed).await;
    stop(&served, json!({ "add": "drop-1" })).await;
    drop.send("page-002.png", b"page two");
    let (status, answer) = body_of(drop.answered().await).await;

    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["cancelled"], true);
    assert!(
        written(&answer).is_empty(),
        "a stopped drop names nothing written"
    );
    assert!(!served.holds("books/vol-1/page-001.png"));
    assert!(!served.holds("books/vol-1/page-002.png"));
    assert!(
        served
            .folder_names("books")
            .iter()
            .all(|name| name != "vol-1"),
        "the folder the drop made is gone with its pages",
    );
    assert!(
        served.holds("books/before.png"),
        "a file that was there before stays"
    );
    assert_eq!(
        served.reservations().await,
        0,
        "the drop's reservation is released"
    );

    let work = work_of(&served).await;
    let undone = stopped_add(&work);
    assert_eq!(undone["files"], 1, "{undone}");
    assert_eq!(undone["removed"], 1, "{undone}");
    assert_eq!(undone["committed"], 0, "{undone}");
    assert_eq!(work["adds"], json!([]), "the add is over once it is undone");
    assert!(work["freeze"].is_null(), "nothing was armed for it: {work}");
}

// LA-36: a drop for a sync stopped while receiving is handed to the sync,
// which undoes it under its turn before it scans anything: the file is
// removed, and the run commits nothing.
#[tokio::test]
async fn a_drop_stopped_while_receiving_is_undone_by_the_sync_before_it_scans() {
    let served = Served::library().await;
    served.plant_locally("albums/before.jpg", b"the person's own");

    let drop = served.add_streamed("albums/2026", "drop-2", false);
    let landed = drop.send("moor.jpg", b"moor");
    served.until_landed(&landed).await;
    stop(&served, json!({ "add": "drop-2" })).await;
    let (status, answer) = body_of(drop.answered().await).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["cancelled"], true);

    served.sync_idle().await;
    assert!(!served.holds("albums/2026/moor.jpg"));
    assert!(served.holds("albums/before.jpg"));
    assert!(!names_in(&served, "albums/2026")
        .await
        .contains(&"moor.jpg".to_owned()));
    let work = work_of(&served).await;
    assert_eq!(sync(&work)["status"], "cancelled", "{work}");
    assert_eq!(stopped_add(&work)["removed"], 1);
    assert!(
        logged(&work).iter().all(|(kind, _)| kind != "sync"),
        "a sync stopped through its add is logged as the add: {work}",
    );
}

// LA-35, LA-36, PK-17: an add whose run is waiting its turn — a book queued behind
// another being packed — is stopped by its own run as that starts, before
// anything of it is read: the book ahead is packed, and this one is undone.
#[tokio::test]
async fn an_add_stopped_while_queued_is_undone_when_its_turn_comes() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.hold_storage();
    served.arm_freeze("scans/vol-1");
    inside_storage(&served).await;

    let (status, answer) = body_of(
        served
            .add_named(
                "books/vol-2",
                "drop-3",
                true,
                &[("page-001.png", b"queued")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(written(&answer), ["books/vol-2/page-001.png"]);
    let work = stop(&served, json!({ "add": "drop-3" })).await;
    assert_eq!(work["adds"][0]["stopping"], true, "{work}");

    served.release_storage();
    served.freeze_idle().await;

    assert!(!served.holds("books/vol-2/page-001.png"));
    assert!(served.holds("scans/vol-1/page-001.jpg"));
    assert_eq!(served.reservations().await, 0);
    assert!(names_in(&served, "books/vol-2").await.is_empty());
    let (_, books) = body_of(served.get("/api/list?path=books").await).await;
    assert!(
        !books.to_string().contains("vol-2"),
        "the listing names nothing of the stopped book: {books}",
    );
    let work = work_of(&served).await;
    assert_eq!(stopped_add(&work)["removed"], 1);
    assert!(
        logged(&work)
            .iter()
            .any(|(kind, entry)| kind == "freeze" && entry["run"]["status"] == "done"),
        "the book ahead of it was packed: {work}",
    );
}

// LA-35, LA-36: a sync stopped while it is running — uploading, before its
// commit — through the Stop on its line ends `cancelled`, and the add it
// carried is undone: nothing committed, the dropped file removed, the file
// that was there before kept.
#[tokio::test]
async fn an_add_stopped_while_running_commits_nothing_and_is_undone() {
    let served = Served::library().await;
    served.plant_locally("albums/before.jpg", b"the person's own");
    served.hold_uploads();

    let (status, _) = body_of(
        served
            .add_named("albums", "drop-4", false, &[("late.jpg", b"late")])
            .await,
    )
    .await;
    assert_eq!(status, 200);
    while served.held_uploads() == 0 {
        tokio::task::yield_now().await;
    }
    let work = work_of(&served).await;
    assert_eq!(sync(&work)["stop"], "offered", "{work}");
    let run = sync(&work)["run"].clone();

    let work = stop(&served, json!({ "sync": run })).await;
    assert_eq!(sync(&work)["stop"], "asked", "{work}");
    served.release_storage();
    served.sync_idle().await;

    assert!(!served.holds("albums/late.jpg"));
    assert!(served.holds("albums/before.jpg"));
    assert!(!names_in(&served, "albums")
        .await
        .contains(&"late.jpg".to_owned()));
    let work = work_of(&served).await;
    assert_eq!(sync(&work)["status"], "cancelled");
    assert_eq!(
        sync(&work)["stop"],
        Value::Null,
        "an ended run offers no Stop"
    );
    let undone = stopped_add(&work);
    assert_eq!(
        (undone["removed"].clone(), undone["committed"].clone()),
        (json!(1), json!(0))
    );

    // What the stopped run spooled and sent is an interrupted run's, and the
    // next run settles it (spec: OC-2, OC-3) — and still adds nothing.
    assert!(
        served.pending_rows().await > 0,
        "the stopped run left its rows"
    );
    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;
    assert_eq!(served.pending_rows().await, 0, "the next run settled them");
    assert!(!names_in(&served, "albums")
        .await
        .contains(&"late.jpg".to_owned()));
}

/// Names in `folder` of the mapped folder that are coffret's own scratch, a
/// file kept aside for an add among them (spec: EP-11, OC-8).
fn scratches_in(served: &Served, folder: &str) -> Vec<String> {
    served
        .folder_names(folder)
        .into_iter()
        .filter(|name| name.starts_with(".coffret-"))
        .collect()
}

/// Puts a file of the person's in the folder and lets a sync carry it in, so
/// it is in the Library and on this device before an add writes over it.
async fn committed_locally(served: &Served, path: &str, content: &[u8]) {
    served.plant_locally(path, content);
    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;
    assert_eq!(sync(&work_of(served).await)["added"], 1);
}

// LA-36, OC-8: an add that wrote over a file of the person's, stopped while
// its sync is running, puts that file back: the path holds what it held
// before, nothing is committed, nothing is left under a scratch name, and the
// next sync finds nothing to carry in.
#[tokio::test]
async fn an_add_that_replaced_a_file_stopped_while_running_puts_that_file_back() {
    let served = Served::library().await;
    committed_locally(&served, "albums/before.jpg", b"the person's own").await;
    let entry = served.entry("albums/before.jpg").await;
    served.hold_uploads();

    let (status, _) = body_of(
        served
            .add_named("albums", "drop-6", false, &[("before.jpg", b"the add's")])
            .await,
    )
    .await;
    assert_eq!(status, 200);
    while served.held_uploads() == 0 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        std::fs::read(served.local_path("albums/before.jpg")).expect("the file is there"),
        b"the add's",
    );
    assert_eq!(
        scratches_in(&served, "albums").len(),
        1,
        "the file it replaced is kept aside while the add runs",
    );
    let run = sync(&work_of(&served).await)["run"].clone();
    stop(&served, json!({ "sync": run })).await;
    served.release_storage();
    served.sync_idle().await;

    assert_eq!(
        std::fs::read(served.local_path("albums/before.jpg")).expect("the file is there"),
        b"the person's own",
        "the file the add replaced is back at its path",
    );
    assert_eq!(scratches_in(&served, "albums"), Vec::<String>::new());
    let work = work_of(&served).await;
    assert_eq!(sync(&work)["status"], "cancelled", "{work}");
    let undone = stopped_add(&work);
    assert_eq!(
        (
            undone["removed"].clone(),
            undone["committed"].clone(),
            undone["left"].clone()
        ),
        (json!(1), json!(0), json!(0)),
        "{undone}",
    );
    assert_eq!(
        served.entry("albums/before.jpg").await,
        entry,
        "nothing committed"
    );

    assert_eq!(served.post("/api/sync").await.status(), 202);
    served.sync_idle().await;
    assert_eq!(sync(&work_of(&served).await)["added"], 0);
    assert_eq!(
        served.entry("albums/before.jpg").await,
        entry,
        "the next sync carries nothing in"
    );
}

// LA-36, OC-8: what an add kept aside of a file it wrote over is let go once
// its run commits: the add's file is the Library's, and nothing is left under
// a scratch name.
#[tokio::test]
async fn the_file_an_add_replaced_is_let_go_once_the_add_commits() {
    let served = Served::library().await;
    committed_locally(&served, "albums/before.jpg", b"the person's own").await;
    let entry = served.entry("albums/before.jpg").await;

    let (status, _) = body_of(
        served
            .add_named("albums", "drop-7", false, &[("before.jpg", b"the add's")])
            .await,
    )
    .await;
    assert_eq!(status, 200);
    served.sync_idle().await;

    assert_eq!(
        std::fs::read(served.local_path("albums/before.jpg")).expect("the file is there"),
        b"the add's",
    );
    assert_eq!(scratches_in(&served, "albums"), Vec::<String>::new());
    assert_ne!(
        served.entry("albums/before.jpg").await,
        entry,
        "the replacement is committed"
    );
    let work = work_of(&served).await;
    assert_eq!(sync(&work)["status"], "done", "{work}");
    assert_eq!(work["adds"], json!([]), "the add is over: {work}");
}

// LA-35: once a run has begun committing it offers no Stop, and a stop asked
// all the same is answered by what the run came to: the file is committed,
// stays on this device, and nothing says it was stopped.
#[tokio::test]
async fn a_stop_after_the_commit_began_is_answered_by_the_commit() {
    let served = Served::library().await;
    served.hold_commits();

    let (status, _) = body_of(
        served
            .add_named("albums", "drop-5", false, &[("late.jpg", b"late")])
            .await,
    )
    .await;
    assert_eq!(status, 200);
    while served.held_commits() == 0 {
        tokio::task::yield_now().await;
    }
    let work = work_of(&served).await;
    assert_eq!(sync(&work)["stop"], "committing", "{work}");

    let work = stop(&served, json!({ "add": "drop-5" })).await;
    assert_eq!(
        sync(&work)["stop"],
        "committing",
        "the stop was not taken: {work}"
    );
    served.release_storage();
    served.sync_idle().await;

    assert!(served.holds("albums/late.jpg"));
    let work = work_of(&served).await;
    assert_eq!(sync(&work)["status"], "done", "{work}");
    assert!(
        logged(&work).iter().all(|(kind, _)| kind != "add"),
        "a stop that met the commit is not logged as a stopped add: {work}",
    );
    assert!(
        rows_of(&served, "albums").await.contains(&(
            "late.jpg".to_owned(),
            "present".to_owned(),
            "one-file".to_owned()
        )),
        "the file is the Library's",
    );
}

// LA-35, PK-7: a freeze of a folder somebody asked to have packed, stopped
// while it is inside Storage, commits nothing: the pages stay where they
// were, none of them in a Pack, and the run is logged as cancelled.
#[tokio::test]
async fn a_freeze_stopped_before_its_commit_packs_nothing() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"one");
    served.plant_locally("scans/vol-1/page-002.jpg", b"two");
    served.hold_storage();
    served.arm_freeze("scans/vol-1");
    inside_storage(&served).await;

    let work = work_of(&served).await;
    assert_eq!(freeze(&work)["stop"], "offered", "{work}");
    stop(&served, json!({ "freeze": freeze(&work)["run"] })).await;
    served.release_storage();
    served.freeze_idle().await;

    assert!(served.holds("scans/vol-1/page-001.jpg"));
    assert!(served.holds("scans/vol-1/page-002.jpg"));
    assert_eq!(
        served.folder_names("scans/vol-1"),
        ["page-001.jpg", "page-002.jpg"],
        "nothing half-written beside them",
    );
    assert!(
        rows_of(&served, "scans/vol-1")
            .await
            .iter()
            .all(|(_, _, container)| container != "pack"),
        "no page of the book is in a Pack",
    );
    let work = work_of(&served).await;
    assert_eq!(freeze(&work)["status"], "cancelled");
    assert!(logged(&work)
        .iter()
        .any(|(kind, entry)| kind == "freeze" && entry["run"]["status"] == "cancelled"));
}

// LA-37, LA-35, PK-21: a fill stopped between two parcels goes no further: what it
// placed stays, each file whole, and nothing half-written is left at a path.
#[tokio::test]
async fn a_fill_stopped_between_parcels_leaves_what_it_placed_whole() {
    let served = Served::library().await;
    let pages: [(&str, usize); 3] = [
        ("scans/vol/000.jpg", 300 * 1024),
        ("scans/vol/001.jpg", 2560 * 1024),
        ("scans/vol/002.jpg", 400 * 1024),
    ];
    for (index, (path, len)) in pages.iter().enumerate() {
        served
            .commit_elsewhere(path, &vec![0x40 + index as u8; *len])
            .await;
    }
    served.pack_elsewhere("scans").await;
    served.start_up().await;

    served.hold_storage_after_parcels(1);
    served.arm_fill("scans/vol");
    inside_storage(&served).await;
    let work = work_of(&served).await;
    assert_eq!(fill(&work)["stop"], "offered", "{work}");
    stop(&served, json!({ "fill": fill(&work)["run"] })).await;
    served.release_storage();
    served.fill_idle().await;

    let work = work_of(&served).await;
    assert_eq!(fill(&work)["status"], "cancelled", "{work}");
    assert!((fill(&work)["done"].as_u64().expect("a count")) < 3);
    for (path, len) in pages {
        let local = served.local_path(path);
        if let Ok(found) = std::fs::metadata(&local) {
            assert_eq!(
                found.len(),
                len as u64,
                "{path} is whole where it is at all"
            );
        }
    }
    assert!(
        served
            .folder_names("scans/vol")
            .iter()
            .all(|name| name.ends_with(".jpg")),
        "no scratch is left beside them: {:?}",
        served.folder_names("scans/vol"),
    );
    assert!(logged(&work)
        .iter()
        .any(|(kind, entry)| kind == "fill" && entry["run"]["status"] == "cancelled"));
}

// LA-34: a deletion cannot be stopped. Its line draws no Stop, and a stop
// naming one is not a stop this route takes.
#[tokio::test]
async fn a_deletion_offers_no_stop() {
    let served = Served::library().await;
    served.hold_storage();
    let (status, work) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202);
    assert!(
        work["delete"].get("stop").is_none(),
        "a deletion's line has nothing to say about a stop: {work}",
    );

    let (status, _) = body_of(served.post_json("/api/stop", &json!({ "delete": 1 })).await).await;
    assert_eq!(status, 400);
    served.release_storage();
    served.delete_idle().await;
}

// LA-35: a stop naming nothing under way changes nothing, and is answered
// with how things stand.
#[tokio::test]
async fn a_stop_of_nothing_under_way_changes_nothing() {
    let served = Served::library().await;
    let work = stop(&served, json!({ "add": "nobody" })).await;
    assert_eq!(work["adds"], json!([]));
    let work = stop(&served, json!({ "freeze": 7 })).await;
    assert!(work["freeze"].is_null());
}

// LA-36: what an import stops is the copy it wrote into the folder; the file
// the person dropped from elsewhere on this device stays where it was.
#[tokio::test]
async fn an_import_stopped_takes_away_its_copies_and_keeps_the_files_dropped() {
    let served = Served::library().await;
    let elsewhere = tempfile::tempdir().expect("a temporary directory must be available");
    let source = elsewhere.path().join("page-001.png");
    std::fs::write(&source, b"the person's page").expect("a file can be written");
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.hold_storage();
    served.arm_freeze("scans/vol-1");
    inside_storage(&served).await;

    let dropped = served.hold_drop(&[&source]).await;
    let (status, answer) = body_of(
        served
            .import(&json!({
                "path": "books/vol-2",
                "freeze": true,
                "add": "import-1",
                "handle": dropped.handle.as_deref().expect("the drop took something"),
                "files": [{ "index": 0, "name": "page-001.png" }],
            }))
            .await,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    stop(&served, json!({ "add": "import-1" })).await;
    served.release_storage();
    served.freeze_idle().await;

    assert!(
        !served.holds("books/vol-2/page-001.png"),
        "the copy is gone"
    );
    assert_eq!(
        std::fs::read(&source).expect("the dropped file is still there"),
        b"the person's page",
    );
    assert_eq!(stopped_add(&work_of(&served).await)["removed"], 1);
}

/// The log's entries of `kind` about `folder`, each as its run.
fn runs_of(work: &Value, kind: &str, folder: &str) -> Vec<Value> {
    logged(work)
        .into_iter()
        .filter(|(logged_kind, entry)| logged_kind == kind && entry["run"]["folder"] == folder)
        .map(|(_, entry)| entry["run"].clone())
        .collect()
}

// LA-34, LA-12: a book somebody asked to have packed, still waiting in the
// freeze's queue behind another, is stopped there: it leaves the queue at
// once, logged cancelled, and nothing of it is ever packed.
#[tokio::test]
async fn a_book_waiting_in_the_freeze_queue_is_stopped_before_its_run() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.plant_locally("scans/vol-2/page-001.jpg", b"the second book");
    served.hold_storage();
    served.arm_freeze("scans/vol-1");
    inside_storage(&served).await;
    served.arm_freeze("scans/vol-2");
    assert_eq!(
        freeze(&work_of(&served).await)["waiting"],
        json!(["scans/vol-2"])
    );

    let work = stop(&served, json!({ "queued_freeze": "scans/vol-2" })).await;
    assert_eq!(freeze(&work)["waiting"], json!([]), "{work}");
    assert_eq!(freeze(&work)["stopping"], json!([]), "{work}");
    let cancelled = runs_of(&work, "freeze", "scans/vol-2");
    assert_eq!(cancelled.len(), 1, "{work}");
    assert_eq!(cancelled[0]["status"], "cancelled");

    served.release_storage();
    served.freeze_idle().await;

    assert!(served.holds("scans/vol-2/page-001.jpg"));
    assert!(
        rows_of(&served, "scans/vol-2")
            .await
            .iter()
            .all(|(_, _, container)| container != "pack"),
        "no page of the stopped book is in a Pack",
    );
    let work = work_of(&served).await;
    assert!(
        runs_of(&work, "freeze", "scans/vol-2")
            .iter()
            .all(|run| run["status"] == "cancelled"),
        "no run of it was taken up: {work}",
    );
    assert!(
        logged(&work).iter().all(|(_, entry)| {
            entry["run"]["status"] != "stopped" || entry["run"]["folder"] != "scans/vol-2"
        }),
        "nothing is logged as failed",
    );
}

// LA-34, LA-36, PK-17: a book waiting in the freeze's queue that holds a drop is
// stopped through that drop: the book says it is stopping until its run takes
// it up, which undoes the drop before reading anything and packs nothing.
#[tokio::test]
async fn a_book_holding_an_add_stopped_in_the_queue_is_undone_by_its_run() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.hold_storage();
    served.arm_freeze("scans/vol-1");
    inside_storage(&served).await;
    let (status, answer) = body_of(
        served
            .add_named(
                "books/vol-2",
                "drop-q",
                true,
                &[("page-001.png", b"queued")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200, "{answer}");

    let work = stop(&served, json!({ "queued_freeze": "books/vol-2" })).await;
    assert_eq!(freeze(&work)["waiting"], json!(["books/vol-2"]), "{work}");
    assert_eq!(freeze(&work)["stopping"], json!(["books/vol-2"]), "{work}");
    assert_eq!(work["adds"][0]["stopping"], true, "{work}");

    served.release_storage();
    served.freeze_idle().await;

    assert!(!served.holds("books/vol-2/page-001.png"));
    assert_eq!(served.reservations().await, 0);
    assert!(names_in(&served, "books/vol-2").await.is_empty());
    let work = work_of(&served).await;
    assert_eq!(stopped_add(&work)["removed"], 1);
    assert!(
        runs_of(&work, "freeze", "books/vol-2").is_empty(),
        "the book is logged as its add and not as itself: {work}",
    );
    assert_eq!(freeze(&work)["stopping"], json!([]));
}

// LA-36, LA-12: a book a drop joined after a run of the whole folder stopped
// is asked for by no selection — the folder's every file — and a Stop on its
// line stops all of it: the drop is undone, and the book is not armed again,
// so the files the folder held before are not packed by a run nobody asked
// for. The pack it was still owed is logged `cancelled` beside the drop.
#[tokio::test]
async fn a_book_stopped_on_its_line_is_not_armed_again_for_the_whole_folder() {
    let served = Served::library().await;
    served.plant_locally("books/vol-2/cover-1.jpg", b"the person's own");
    served.plant_locally("books/vol-2/cover-2.jpg", b"and another");
    served.halt_storage();
    served.arm_freeze("books/vol-2");
    served.freeze_idle().await;
    assert_eq!(freeze(&work_of(&served).await)["status"], "stopped");

    served.resume_storage();
    served.hold_storage();
    let (status, answer) = body_of(
        served
            .add_named(
                "books/vol-2",
                "drop-w",
                true,
                &[("page-001.png", b"dropped")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    inside_storage(&served).await;
    let work = work_of(&served).await;
    assert_eq!(freeze(&work)["folder"], "books/vol-2", "{work}");
    assert_eq!(freeze(&work)["stop"], "offered", "{work}");

    stop(&served, json!({ "freeze": freeze(&work)["run"] })).await;
    served.release_storage();
    served.freeze_idle().await;

    assert!(!served.holds("books/vol-2/page-001.png"));
    assert!(served.holds("books/vol-2/cover-1.jpg"));
    assert!(served.holds("books/vol-2/cover-2.jpg"));
    assert!(
        rows_of(&served, "books/vol-2")
            .await
            .iter()
            .all(|(_, _, container)| container != "pack"),
        "nothing of the folder is in a Pack",
    );
    let work = work_of(&served).await;
    assert_eq!(stopped_add(&work)["removed"], 1);
    assert_eq!(
        freeze(&work)["status"],
        "cancelled",
        "no run after it: {work}"
    );
    assert_eq!(freeze(&work)["waiting"], json!([]), "{work}");
    let cancelled = runs_of(&work, "freeze", "books/vol-2");
    assert_eq!(
        cancelled
            .iter()
            .filter(|run| run["status"] == "cancelled")
            .count(),
        1,
        "the book still owed a pack is logged cancelled beside its add: {work}",
    );
}

// LA-12, LA-35: a book somebody asked to have packed, joined by a drop while
// it waits in the freeze's queue, stopped on its line there, is logged as the
// stopped drop and as a `cancelled` freeze of its folder, so whoever asked to
// have it packed sees what became of that request; nothing of it is packed.
#[tokio::test]
async fn a_requested_book_holding_an_add_stopped_in_the_queue_is_logged_cancelled_too() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.plant_locally("books/vol-2/cover.jpg", b"the person's own");
    served.hold_storage();
    served.arm_freeze("scans/vol-1");
    inside_storage(&served).await;
    served.arm_freeze("books/vol-2");
    let (status, answer) = body_of(
        served
            .add_named(
                "books/vol-2",
                "drop-r",
                true,
                &[("page-001.png", b"joined")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        freeze(&work_of(&served).await)["waiting"],
        json!(["books/vol-2"]),
        "the drop joined the book waiting",
    );

    let work = stop(&served, json!({ "queued_freeze": "books/vol-2" })).await;
    assert_eq!(freeze(&work)["stopping"], json!(["books/vol-2"]), "{work}");

    served.release_storage();
    served.freeze_idle().await;

    assert!(!served.holds("books/vol-2/page-001.png"));
    assert!(served.holds("books/vol-2/cover.jpg"));
    assert!(
        rows_of(&served, "books/vol-2")
            .await
            .iter()
            .all(|(_, _, container)| container != "pack"),
        "nothing of the folder is in a Pack",
    );
    let work = work_of(&served).await;
    assert_eq!(stopped_add(&work)["removed"], 1, "{work}");
    let runs = runs_of(&work, "freeze", "books/vol-2");
    assert_eq!(runs.len(), 1, "{work}");
    assert_eq!(runs[0]["status"], "cancelled", "{work}");
    assert_eq!(
        freeze(&work)["waiting"],
        json!([]),
        "not armed again: {work}"
    );
}

/// The pages of the book a drop cut off by its server stopping received.
const CUT_OFF: [(&str, &[u8]); 2] = [
    ("page-001.jpg", b"the first page"),
    ("page-002.jpg", b"the second page"),
];

// PK-17, LA-36: a drop that took over the reservation of one cut off by its
// server stopping, stopped while receiving, takes away what it wrote and
// keeps that reservation, for the pages the cut-off drop received.
#[tokio::test]
async fn a_drop_that_took_a_cut_off_one_over_keeps_its_reservation_when_stopped() {
    let mut served = Served::library().await;
    let first = served.upload_book_streamed("scans/vol-1");
    for (name, content) in &CUT_OFF {
        served.until_landed(&first.send(name, content)).await;
    }
    first.abandon();
    served.restart();
    served.start_up().await;
    served.freeze_idle().await;

    let drop = served.add_streamed("scans/vol-1", "drop-t", true);
    served
        .until_landed(&drop.send("page-003.jpg", b"the third page"))
        .await;
    stop(&served, json!({ "add": "drop-t" })).await;
    let (status, answer) = body_of(drop.answered().await).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["cancelled"], true, "{answer}");

    assert!(!served.holds("scans/vol-1/page-003.jpg"));
    for (name, _) in &CUT_OFF {
        assert!(
            served.holds(&format!("scans/vol-1/{name}")),
            "the cut-off drop's {name} stays"
        );
    }
    let reserved = served.reserved_paths().await;
    assert_eq!(reserved.len(), 1, "the reservation is kept: {reserved:?}");
    for (name, _) in &CUT_OFF {
        assert!(
            reserved[0].contains(&format!("scans/vol-1/{name}")),
            "kept for the cut-off drop's {name}: {reserved:?}",
        );
    }
    assert_eq!(stopped_add(&work_of(&served).await)["removed"], 1);
}

// LA-34, LA-12: a folder waiting in the fill's queue behind another is
// stopped there: it leaves the queue at once, logged cancelled, and nothing of
// it is brought over.
#[tokio::test]
async fn a_folder_waiting_in_the_fill_queue_is_stopped_before_its_run() {
    let served = Served::library().await;
    served.commit_elsewhere("albums/a.jpg", b"first").await;
    served.commit_elsewhere("letters/b.jpg", b"second").await;
    served.start_up().await;
    served.hold_storage();
    served.arm_fill("albums");
    inside_storage(&served).await;
    served.queue_fill("letters");
    assert_eq!(fill(&work_of(&served).await)["waiting"], json!(["letters"]));

    let work = stop(&served, json!({ "queued_fill": "letters" })).await;
    assert_eq!(fill(&work)["waiting"], json!([]), "{work}");
    let cancelled = runs_of(&work, "fill", "letters");
    assert_eq!(cancelled.len(), 1, "{work}");
    assert_eq!(cancelled[0]["status"], "cancelled");
    assert_eq!(cancelled[0]["done"], 0);

    served.release_storage();
    served.fill_idle().await;

    assert!(
        !served.holds("letters/b.jpg"),
        "nothing of it was brought over"
    );
    assert!(served.holds("albums/a.jpg"), "the folder ahead of it was");
    let work = work_of(&served).await;
    assert_eq!(runs_of(&work, "fill", "letters").len(), 1, "{work}");
}

// LA-35: an add's name is a short token — one naming anything else is refused
// as a bad request and writes nothing — and a stop naming no such add, a name
// of that shape or not, changes nothing.
#[tokio::test]
async fn an_add_is_named_by_a_short_token() {
    let served = Served::library().await;
    let (status, refusal) = body_of(
        served
            .add_named("albums", &"x".repeat(65), false, &[("a.jpg", b"one")])
            .await,
    )
    .await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");
    assert!(!served.holds("albums/a.jpg"), "nothing was written");

    let work = stop(&served, json!({ "add": "not a name" })).await;
    assert_eq!(work["adds"], json!([]));
    let work = stop(&served, json!({ "add": "drop-unknown" })).await;
    assert_eq!(work["adds"], json!([]));
}
