//! Moving files and folders in the Library: what `GET /api/move` says a set of
//! pairs would do, and what `POST /api/move` then does (spec: FM-15, EP-10).

use serde_json::json;

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
// is logged as finished work.
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

    let (status, moved) = body_of(served.post(named).await).await;
    assert_eq!(status, 200, "{moved}");
    assert_eq!(moved["status"], "done");
    assert_eq!(moved["entries"], 2);
    assert_eq!(moved["relocated"], 2);
    assert_eq!(moved["left"], 0);
    assert_eq!(moved["stopped"], serde_json::Value::Null);
    assert_eq!(moved["unrelocated"], serde_json::Value::Null);

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
    let entries = log(&served).await;
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0]["kind"], "move");
    assert_eq!(entries[0]["run"]["status"], "done");
    assert_eq!(entries[0]["run"]["entries"], 2);
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

    let (status, moved) = body_of(served.post(named).await).await;
    assert_eq!(status, 200, "{moved}");
    assert_eq!(moved["status"], "done");
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
// by the preview as refusals, and refuse the move before anything is written.
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
    let (status, refusal) = body_of(served.post(named).await).await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["error"], "refused_move");
    assert_eq!(refusal["reason"], "edited");

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
    let (status, refusal) = body_of(served.post(named).await).await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["error"], "refused_move");

    assert!(served.holds("albums/2026/summer.jpg"), "nothing moved");
    assert!(!served.holds("books/2026/summer.jpg"));
    assert!(log(&served).await.is_empty(), "a refused move did nothing");
}

// LA-30, LA-31: a set the Library does not admit is the preview's refusal and the
// move's, in the shape every refusal takes; a query that is no set of pairs is
// refused before the Library is asked.
#[tokio::test]
async fn a_set_the_library_does_not_admit_is_refused() {
    let served = Served::library().await;
    let overlapping = "/api/move?from=albums/cover.png&to=kept&from=albums/2026&to=kept/2026";

    let (status, preview) = body_of(served.get(overlapping).await).await;
    assert_eq!(status, 200, "{preview}");
    assert_eq!(preview["entries"], 0);
    assert_eq!(preview["refused"][0]["reason"], "overlapping", "{preview}");
    let (status, refusal) = body_of(served.post(overlapping).await).await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["reason"], "overlapping");

    let (status, refusal) = body_of(
        served
            .post("/api/move?from=albums/gone.jpg&to=books/gone.jpg")
            .await,
    )
    .await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["reason"], "nothing_to_move");

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
// before anything is written, so it is answered as the preview's refusals are
// and not logged.
#[tokio::test]
async fn a_set_refused_after_the_preview_admitted_it_is_answered_and_not_logged() {
    let served = with_albums_here(Served::library().await).await;
    let named = "/api/move?from=albums/2026&to=books/2026";
    // A head the move's preview has to read a record to reach, so the
    // preview's catch-up can be held inside that read: it has listed the
    // Library by then, and the commit made while it waits is one only the
    // move's own catch-up sees.
    served.commit_elsewhere("elsewhere/one.jpg", b"one").await;
    served.hold_next_whole_read();
    let (answer, ()) = tokio::join!(served.post(named), async {
        while served.held_reads() == 0 {
            tokio::task::yield_now().await;
        }
        served
            .commit_elsewhere("books/2026/spring.jpg", b"taken")
            .await;
        served.release_storage();
    });
    let (status, refusal) = body_of(answer).await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["error"], "refused_move");
    assert_eq!(refusal["reason"], "destination_taken", "{refusal}");
    assert!(served.holds("albums/2026/summer.jpg"), "nothing moved");
    assert!(log(&served).await.is_empty(), "a refused move did nothing");
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
    let (answer, ()) = tokio::join!(served.post(named), async {
        while served.held_reads() == 0 {
            tokio::task::yield_now().await;
        }
        served.refuse_the_catalog();
        served.release_storage();
    });
    let (status, moved) = body_of(answer).await;
    assert_eq!(status, 200, "{moved}");
    assert_eq!(moved["status"], "done");
    assert_eq!(moved["entries"], 2);
    assert_eq!(moved["relocated"], 0);
    assert_eq!(moved["stopped"], serde_json::Value::Null);
    assert_eq!(moved["unrelocated"]["error"], "server", "{moved}");
    assert!(
        served.holds("albums/2026/spring.jpg"),
        "the file stays until the next sync or fetch relocates it"
    );

    let entries = log(&served).await;
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0]["run"]["status"], "done");
    assert_eq!(entries[0]["run"]["unrelocated"], moved["unrelocated"]);
}
