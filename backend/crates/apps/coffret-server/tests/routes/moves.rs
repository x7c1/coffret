//! Moving files and folders in the Library: what `GET /api/move` says a set of
//! pairs would do, and what `POST /api/move` then does as background work
//! (spec: FM-15, EP-10, LA-31).

use serde_json::{json, Value};

use crate::support::{json as body_of, route, rows_of, Served};

/// The moves in the work answer's log, newest first.
async fn log(served: &Served) -> Vec<serde_json::Value> {
    let (_, work) = body_of(served.get("/api/work").await).await;
    work["log"]
        .as_array()
        .expect("the work answer carries the log")
        .iter()
        .filter(|entry| entry["kind"] == "move")
        .cloned()
        .collect()
}

/// Asks for a move of what `named` names, which the server accepts at once,
/// and waits for it to end: what the log says it came to.
async fn moved(served: &Served, named: &str) -> Value {
    let (status, accepted) = body_of(served.post(named).await).await;
    assert_eq!(status, 202, "{accepted}");
    let run = accepted["run"].clone();
    served.move_idle().await;
    ended(served, &run).await
}

/// The log entry of move `run`, which has ended.
async fn ended(served: &Served, run: &Value) -> Value {
    log(served)
        .await
        .into_iter()
        .find(|entry| &entry["run"]["run"] == run)
        .unwrap_or_else(|| panic!("move {run} is in the log"))["run"]
        .clone()
}

/// The moves the work answer lists as pending, and the answer itself.
async fn pending(served: &Served) -> (Vec<Value>, Value) {
    let (_, work) = body_of(served.get("/api/work").await).await;
    let moves = work["moves"]
        .as_array()
        .expect("the work answer lists the pending moves")
        .clone();
    (moves, work)
}

/// Waits until a run just armed is inside Storage, and so holds this device's
/// turn at its pending rows.
async fn inside_storage(served: &Served) {
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
}

/// A server over a device that has brought `albums/2026` over from the
/// Library, so its own files of the two Entries there are on its disk.
async fn with_albums_here(served: Served) -> Served {
    served.arm_fill("albums/2026");
    served.fill_idle().await;
    served
}

// LA-30, LA-31, LA-12: a folder moved within the one mapped folder is
// previewed as two Entries whose two files follow, and then moved in one
// record: the files follow on disk, nothing leaves this device, and the move
// is logged as finished work with what it did.
#[tokio::test]
async fn a_folder_moved_within_the_mapped_folder_is_previewed_and_its_files_follow() {
    let served = with_albums_here(Served::library().await).await;
    assert!(served.holds("albums/2026/spring.jpg"));
    let named = "/api/move?from=albums/2026&to=books/2026";

    let (status, preview) = body_of(served.get(named).await).await;
    assert_eq!(status, 200, "{preview}");
    assert_eq!(
        preview["pairs"],
        json!([{ "from": "albums/2026", "to": "books/2026" }])
    );
    assert_eq!(preview["entries"], 2);
    assert_eq!(preview["following"], 2, "{preview}");
    assert_eq!(preview["leaving"], 0);
    assert_eq!(preview["leaving_bytes"], 0);
    assert_eq!(preview["edited"], json!([]));
    assert_eq!(preview["uploading"], json!([]));
    assert_eq!(preview["refused"], json!([]));
    assert!(log(&served).await.is_empty(), "a preview does nothing");
    assert!(served.holds("albums/2026/spring.jpg"), "and moves nothing");

    let moved = moved(&served, named).await;
    assert_eq!(moved["status"], "done", "{moved}");
    assert_eq!(moved["entries"], 2);
    assert_eq!(moved["relocated"], 2);
    assert_eq!(moved["left"], 0);
    assert_eq!(moved["stopped"], Value::Null);
    assert_eq!(moved["unrelocated"], Value::Null);

    for name in ["spring.jpg", "summer.jpg"] {
        assert!(!served.holds(&format!("albums/2026/{name}")));
        assert!(
            served.holds(&format!("books/2026/{name}")),
            "{name} follows"
        );
    }
    assert_eq!(
        rows_of(&served, "books/2026").await,
        ["spring.jpg", "summer.jpg"].map(|name| (
            name.to_owned(),
            "present".to_owned(),
            "one-file".to_owned()
        )),
    );
    assert_eq!(log(&served).await.len(), 1, "logged once");
    let (moves, _) = pending(&served).await;
    assert!(moves.is_empty(), "and no longer pending: {moves:?}");
}

// LA-30, LA-31: a folder moved to where this device maps no folder cannot take this
// device's files along. The preview counts them as leaving, with their size,
// and the move discards the unedited files into the trash: the Entries stand
// at their new paths in the Library, and not on this device.
#[tokio::test]
async fn a_folder_moved_to_where_this_device_maps_nothing_leaves_the_device() {
    let served = with_albums_here(Served::mapping_only("albums").await).await;
    let spring = served.local_root().join("2026/spring.jpg");
    assert!(spring.is_file());
    let named = "/api/move?from=albums/2026&to=books/2026";

    let (status, preview) = body_of(served.get(named).await).await;
    assert_eq!(status, 200, "{preview}");
    assert_eq!(preview["entries"], 2);
    assert_eq!(preview["following"], 0);
    assert_eq!(preview["leaving"], 2, "{preview}");
    assert_eq!(
        preview["leaving_bytes"],
        b"spring".len() + b"summer".len(),
        "{preview}"
    );
    assert_eq!(preview["refused"], json!([]));

    let moved = moved(&served, named).await;
    assert_eq!(moved["status"], "done", "{moved}");
    assert_eq!(moved["relocated"], 0);
    assert_eq!(moved["left"], 2, "{moved}");
    let findings = moved["findings"].as_array().expect("a list of findings");
    assert_eq!(findings.len(), 2, "{moved}");
    for finding in findings {
        assert_eq!(finding["surfaced"], "RelocationLeft", "{finding}");
    }
    assert!(!spring.exists(), "the file left this device");

    let (status, file) = body_of(served.get("/api/list?path=books/2026").await).await;
    assert_eq!(status, 200, "{file}");
    let states: Vec<&str> = file["files"]
        .as_array()
        .expect("a listing has files")
        .iter()
        .map(|row| row["state"].as_str().expect("a state"))
        .collect();
    assert_eq!(states, ["remote", "remote"], "{file}");
}

// LA-30, LA-31: a file of this device's in the move that changed since it was
// placed, and a file under it the Library does not hold yet, are both shown
// by the preview as refusals, and stop the move, once accepted, before
// anything is written: it is logged as stopped with that refusal.
#[tokio::test]
async fn an_edited_file_and_an_uploading_one_refuse_the_move() {
    let served = with_albums_here(Served::library().await).await;
    let named = "/api/move?from=albums/2026&to=books/2026";
    served.plant_locally("albums/2026/spring.jpg", b"spring, edited since");

    let (status, preview) = body_of(served.get(named).await).await;
    assert_eq!(status, 200, "{preview}");
    assert_eq!(preview["edited"], json!(["albums/2026/spring.jpg"]));
    assert_eq!(preview["refused"][0]["error"], "refused_move");
    assert_eq!(preview["refused"][0]["reason"], "edited", "{preview}");
    let stopped = moved(&served, named).await;
    assert_eq!(stopped["status"], "stopped", "{stopped}");
    assert_eq!(stopped["stopped"]["error"], "refused_move");
    assert_eq!(stopped["stopped"]["reason"], "edited");
    assert_eq!(stopped["entries"], 0);

    served.plant_locally("albums/2026/spring.jpg", b"spring");
    served.plant_locally("albums/2026/autumn.jpg", b"not in the Library yet");
    let (status, preview) = body_of(served.get(named).await).await;
    assert_eq!(status, 200, "{preview}");
    assert_eq!(preview["uploading"], json!(["albums/2026/autumn.jpg"]));
    let reasons: Vec<&str> = preview["refused"]
        .as_array()
        .expect("a list of refusals")
        .iter()
        .map(|refusal| refusal["reason"].as_str().expect("a reason"))
        .collect();
    assert!(reasons.contains(&"uploading"), "{preview}");
    let stopped = moved(&served, named).await;
    assert_eq!(stopped["status"], "stopped", "{stopped}");
    assert_eq!(stopped["stopped"]["error"], "refused_move");
    assert_eq!(stopped["stopped"]["reason"], "uploading");

    assert!(served.holds("albums/2026/summer.jpg"), "nothing moved");
    assert!(!served.holds("books/2026/summer.jpg"));
    assert_eq!(
        rows_of(&served, "books/2026").await,
        Vec::<(String, String, String)>::new(),
        "and nothing was committed"
    );
}

// LA-30, LA-31: a set the Library does not admit is the preview's refusal and
// what stops the move, in the shape every refusal takes; a query that is no set
// of pairs is refused before anything is accepted.
#[tokio::test]
async fn a_set_the_library_does_not_admit_is_refused() {
    let served = Served::library().await;
    let overlapping = "/api/move?from=albums/cover.png&to=kept&from=albums/2026&to=kept/2026";

    let (status, preview) = body_of(served.get(overlapping).await).await;
    assert_eq!(status, 200, "{preview}");
    assert_eq!(preview["entries"], 0);
    assert_eq!(preview["refused"][0]["reason"], "overlapping", "{preview}");
    let stopped = moved(&served, overlapping).await;
    assert_eq!(stopped["stopped"]["error"], "refused_move", "{stopped}");
    assert_eq!(stopped["stopped"]["reason"], "overlapping");

    let stopped = moved(&served, "/api/move?from=albums/gone.jpg&to=books/gone.jpg").await;
    assert_eq!(stopped["stopped"]["reason"], "nothing_to_move", "{stopped}");

    for unpaired in [
        "/api/move",
        "/api/move?from=albums",
        "/api/move?from=a&to=b&to=c",
    ] {
        for method in ["GET", "POST"] {
            let (status, refusal) = route(&served, method, unpaired).await;
            assert_eq!(status, 400, "{method} {unpaired}: {refusal}");
            assert_eq!(refusal["error"], "bad_path");
        }
    }
}

// LA-30, LA-31: both refused while the Library is locked, before anything is
// read (spec: DK-2).
#[tokio::test]
async fn a_move_and_its_preview_are_refused_while_locked() {
    let served = Served::library().await;
    served.lock();
    for method in ["GET", "POST"] {
        let (status, refusal) =
            route(&served, method, "/api/move?from=albums/2026&to=books/2026").await;
        assert_eq!(status, 423, "{method}: {refusal}");
        assert_eq!(refusal["error"], "locked");
    }
}

// LA-31: the preview admitted the set, and another device then took a
// destination before the move's own catch-up. The rename's plan refuses it
// before anything is written, so it is logged as stopped with the refusal the
// preview would have shown.
#[tokio::test]
async fn a_set_refused_after_the_preview_admitted_it_is_logged_as_stopped() {
    let served = with_albums_here(Served::library().await).await;
    let named = "/api/move?from=albums/2026&to=books/2026";
    // A head the move's catch-up has to read a record to reach, so it can be
    // held inside that read: the move's preview has listed the Library by
    // then, and the commit made while it waits is one only the rename's own
    // catch-up sees.
    served.commit_elsewhere("elsewhere/one.jpg", b"one").await;
    served.hold_next_whole_read();
    let (status, accepted) = body_of(served.post(named).await).await;
    assert_eq!(status, 202, "{accepted}");
    inside_storage(&served).await;
    served
        .commit_elsewhere("books/2026/spring.jpg", b"taken")
        .await;
    served.release_storage();
    served.move_idle().await;

    let stopped = ended(&served, &accepted["run"]).await;
    assert_eq!(stopped["status"], "stopped", "{stopped}");
    assert_eq!(stopped["stopped"]["error"], "refused_move");
    assert_eq!(
        stopped["stopped"]["reason"], "destination_taken",
        "{stopped}"
    );
    assert!(served.holds("albums/2026/summer.jpg"), "nothing moved");
}

// LA-31, LA-12: the record committed, and this device's catalog then could
// not say which of its files wait to follow it. The move is logged as done —
// its record holds the renames — with what kept the files where they were,
// rather than as a move that stopped and committed nothing.
#[tokio::test]
async fn a_move_whose_files_could_not_follow_after_its_commit_is_logged_as_done() {
    let served = with_albums_here(Served::library().await).await;
    let named = "/api/move?from=albums/2026&to=books/2026";
    // The first whole read the move makes is its commit's read of the
    // Keyring, after the plan: the catalog is taken away there, so the
    // relocations after the commit are what meet it.
    served.hold_next_whole_read();
    let (status, accepted) = body_of(served.post(named).await).await;
    assert_eq!(status, 202, "{accepted}");
    inside_storage(&served).await;
    served.refuse_the_catalog();
    served.release_storage();
    served.move_idle().await;
    let moved = ended(&served, &accepted["run"]).await;
    assert_eq!(moved["status"], "done", "{moved}");
    assert_eq!(moved["entries"], 2);
    assert_eq!(moved["relocated"], 0);
    assert_eq!(moved["stopped"], Value::Null);
    assert_eq!(moved["unrelocated"]["error"], "server", "{moved}");
    assert!(
        served.holds("albums/2026/spring.jpg"),
        "the file stays until the next sync or fetch relocates it"
    );
}

// LA-31, LA-12: a move asked for while a deletion holds this device's turn is
// answered at once, before its commit: it is listed as pending and waiting for
// that turn, nothing has moved, and nothing is in the log. Once the turn is
// given back it is carried out, and it leaves the pending moves for the log,
// with what it did.
#[tokio::test]
async fn a_move_is_answered_before_its_commit_and_then_carried_out_in_its_turn() {
    let served = with_albums_here(Served::library().await).await;
    served.hold_storage();
    let (status, _) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202);
    inside_storage(&served).await;

    let named = "/api/move?from=albums/2026&to=books/2026";
    let (status, accepted) = body_of(served.post(named).await).await;
    assert_eq!(status, 202, "{accepted}");
    let run = accepted["run"].clone();
    assert_eq!(run, 1, "{accepted}");
    assert_eq!(accepted["work"]["moves"][0]["run"], run, "{accepted}");

    served.until_queued(1).await;
    let (moves, work) = pending(&served).await;
    assert_eq!(work["turn"], "delete", "{work}");
    assert_eq!(
        moves,
        [json!({
            "run": 1,
            "pairs": [{ "from": "albums/2026", "to": "books/2026" }],
            "waits": true,
        })],
    );
    assert!(log(&served).await.is_empty(), "nothing has ended: {work}");
    assert!(served.holds("albums/2026/spring.jpg"), "nor moved");

    served.release_storage();
    served.delete_idle().await;
    served.move_idle().await;

    let (moves, _) = pending(&served).await;
    assert!(moves.is_empty(), "{moves:?}");
    let moved = ended(&served, &run).await;
    assert_eq!(moved["status"], "done", "{moved}");
    assert_eq!(
        moved["pairs"],
        json!([{ "from": "albums/2026", "to": "books/2026" }])
    );
    assert_eq!(moved["entries"], 2);
    assert_eq!(moved["relocated"], 2);
    assert_eq!(moved["left"], 0);
    assert!(served.holds("books/2026/spring.jpg"), "the file followed");
}

// LA-31, LA-12: the preview admitted the move, and a file of it was edited on
// this device while the move waited for its turn. Asked again under that turn,
// the edited file stops it: nothing is committed or moved, and the log says
// it stopped and why.
#[tokio::test]
async fn a_move_refused_under_its_turn_commits_nothing_and_is_logged_with_its_refusal() {
    let served = with_albums_here(Served::library().await).await;
    let named = "/api/move?from=albums/2026&to=books/2026";
    let (_, preview) = body_of(served.get(named).await).await;
    assert_eq!(preview["refused"], json!([]), "{preview}");

    served.hold_storage();
    let (status, _) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202);
    inside_storage(&served).await;
    let (status, accepted) = body_of(served.post(named).await).await;
    assert_eq!(status, 202, "{accepted}");
    served.plant_locally("albums/2026/spring.jpg", b"spring, edited while it waited");
    served.release_storage();
    served.delete_idle().await;
    served.move_idle().await;

    let stopped = ended(&served, &accepted["run"]).await;
    assert_eq!(stopped["status"], "stopped", "{stopped}");
    assert_eq!(stopped["stopped"]["error"], "refused_move");
    assert_eq!(stopped["stopped"]["reason"], "edited");
    assert_eq!(stopped["entries"], 0);
    assert!(served.holds("albums/2026/spring.jpg"), "nothing moved");
    assert!(!served.holds("books/2026/spring.jpg"));
    assert_eq!(
        rows_of(&served, "books/2026").await,
        Vec::<(String, String, String)>::new(),
        "and nothing was committed"
    );
    assert_eq!(rows_of(&served, "albums/2026").await.len(), 2);
}

// LA-31: while a move is pending, a second move of the same rows — the same
// set, one under it, its destination — is refused up front, and so is its
// preview; a deletion of them and a drop among them are refused as a
// conflict. A move of other rows is accepted, behind it.
#[tokio::test]
async fn what_touches_the_rows_of_a_pending_move_is_refused_up_front() {
    let served = with_albums_here(Served::library().await).await;
    // The deletion is held at its commit, so it holds the turn while the
    // Library can still be read: the previews below catch up as they ask.
    served.hold_commits();
    let (status, _) = body_of(served.post("/api/delete?entry=albums/notes.txt").await).await;
    assert_eq!(status, 202);
    while served.held_commits() == 0 {
        tokio::task::yield_now().await;
    }
    let (status, accepted) = body_of(
        served
            .post("/api/move?from=albums/2026&to=books/2026")
            .await,
    )
    .await;
    assert_eq!(status, 202, "{accepted}");

    for touching in [
        "/api/move?from=albums/2026&to=books/2026",
        "/api/move?from=albums/2026/spring.jpg&to=albums/spring.jpg",
        "/api/move?from=albums/cover.png&to=books/2026/cover.png",
    ] {
        let (status, preview) = body_of(served.get(touching).await).await;
        assert_eq!(status, 200, "{touching}: {preview}");
        assert_eq!(preview["refused"][0]["reason"], "moving", "{preview}");
        let (status, refusal) = body_of(served.post(touching).await).await;
        assert_eq!(status, 409, "{touching}: {refusal}");
        assert_eq!(refusal["error"], "refused_move");
        assert_eq!(refusal["reason"], "moving", "{refusal}");
        assert!(
            refusal["message"]
                .as_str()
                .expect("a sentence")
                .contains("is under way"),
            "{refusal}"
        );
    }
    for touching in [
        "/api/delete?path=albums",
        "/api/delete?entry=books/2026/a.jpg",
    ] {
        let (status, refusal) = body_of(served.post(touching).await).await;
        assert_eq!(status, 409, "{touching}: {refusal}");
        assert_eq!(refusal["error"], "conflict", "{refusal}");
    }
    let (status, refusal) =
        body_of(served.upload("books/2026", &[("late.jpg", b"late")]).await).await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["error"], "conflict", "{refusal}");

    let (status, other) = body_of(
        served
            .post("/api/move?from=albums/cover.png&to=books/cover.png")
            .await,
    )
    .await;
    assert_eq!(status, 202, "{other}");
    let (moves, _) = pending(&served).await;
    assert_eq!(moves.len(), 2, "{moves:?}");

    served.release_storage();
    served.delete_idle().await;
    served.move_idle().await;
    assert!(served.holds("books/2026/spring.jpg"));
    for run in [&accepted["run"], &other["run"]] {
        assert_eq!(ended(&served, run).await["status"], "done");
    }
}
