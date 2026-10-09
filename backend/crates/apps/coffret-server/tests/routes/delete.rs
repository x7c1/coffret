//! Taking files and folders out of the Library: the count `GET /api/delete`
//! answers before anything is asked for, and the run `POST /api/delete` arms
//! once somebody has said yes (spec: PK-9, PK-10).

use serde_json::json;

use crate::support::{bytes, deletion, json as body_of, rows_of, Served};

/// The three files of the Pack the other device packs, and what is in each.
const SCANS: [(&str, &[u8]); 3] = [
    ("scans/a.jpg", b"the first scan"),
    ("scans/b.jpg", b"the second scan"),
    ("scans/c.jpg", b"the third scan"),
];

/// A server whose Library holds `scans/` as one Pack of three files, committed
/// by the other device and never fetched here, beside the fixture's one-file
/// Entries.
async fn with_a_pack() -> Served {
    let served = Served::library().await;
    for (path, content) in SCANS {
        served.commit_elsewhere(path, content).await;
    }
    served.pack_elsewhere("scans").await;
    assert_eq!(served.post("/api/refresh").await.status(), 200);
    served
}

// The whole of what the preview is for: what it counts is what the run then
// does. One file inside a Pack that keeps two others, and one file that is a
// one-file Container of its own — so the count has a removal and a rebuild in
// it, and the bytes a rebuild costs.
#[tokio::test]
async fn the_preview_counts_exactly_what_the_deletion_does() {
    let served = with_a_pack().await;
    let named = "/api/delete?entry=scans/b.jpg&entry=albums/cover.png";

    let (status, preview) = body_of(served.get(named).await).await;
    assert_eq!(status, 200, "{preview}");
    assert_eq!(preview["folder"], serde_json::Value::Null);
    assert_eq!(preview["paths"], json!(["albums/cover.png", "scans/b.jpg"]));
    assert_eq!(preview["entries"], 2);
    assert_eq!(
        preview["bytes"],
        b"the second scan".len() + b"cover".len(),
        "{preview}"
    );
    assert_eq!(preview["removed"], 1, "the cover's own Container goes");
    assert_eq!(preview["rebuilt"], 1, "the Pack is rebuilt around a and c");
    let read = preview["rebuild_read"].as_u64().expect("a byte count");
    let written = preview["rebuild_written"].as_u64().expect("a byte count");
    assert!(read > 0 && written > 0, "{preview}");
    assert_eq!(preview["refused"], json!([]));
    assert_eq!(preview["missing"], json!([]));
    assert_eq!(preview["after_current"], false);

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(
        work["delete"],
        serde_json::Value::Null,
        "a preview arms nothing"
    );

    let (status, armed) = body_of(served.post(named).await).await;
    assert_eq!(status, 202, "{armed}");
    assert_eq!(deletion(&armed)["status"], "deleting");
    served.delete_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let done = deletion(&work);
    assert_eq!(done["status"], "done", "{work}");
    for count in [
        "entries",
        "bytes",
        "removed",
        "rebuilt",
        "rebuild_read",
        "rebuild_written",
        "refused",
        "missing",
    ] {
        assert_eq!(done[count], preview[count], "{count} as counted: {work}");
    }
    assert_eq!(done["stopped"], serde_json::Value::Null);
    assert_eq!(done["step"], serde_json::Value::Null);

    // The Library no longer lists them, and the rest of the Pack is still
    // there — and still readable, out of the rebuilt Pack, by a device that
    // never had it.
    assert_eq!(
        rows_of(&served, "scans").await,
        ["a.jpg", "c.jpg"].map(|name| (name.to_owned(), "remote".to_owned(), "pack".to_owned())),
    );
    assert!(!rows_of(&served, "albums")
        .await
        .iter()
        .any(|(name, _, _)| name == "cover.png"));
    let response = served.get("/api/file?path=scans/c.jpg").await;
    assert_eq!(response.status(), 200);
    assert_eq!(bytes(response).await, b"the third scan");
}

// A folder is everything under it, at every depth (spec: EP-9): here five
// one-file Containers, each removed outright.
#[tokio::test]
async fn a_folder_is_deleted_with_everything_under_it() {
    let served = Served::library().await;

    let (status, preview) = body_of(served.get("/api/delete?path=albums").await).await;
    assert_eq!(status, 200, "{preview}");
    assert_eq!(preview["folder"], "albums");
    assert_eq!(preview["paths"], json!([]));
    assert_eq!(preview["entries"], 5);
    assert_eq!(preview["removed"], 5);
    assert_eq!(preview["rebuilt"], 0);
    assert_eq!(preview["rebuild_read"], 0);

    let (status, _) = body_of(served.post("/api/delete?path=albums").await).await;
    assert_eq!(status, 202);
    served.delete_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(deletion(&work)["status"], "done", "{work}");
    assert_eq!(deletion(&work)["folder"], "albums");
    assert_eq!(deletion(&work)["entries"], 5);

    let (status, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        listing["held"], false,
        "the Library holds nothing there now"
    );
    assert_eq!(
        rows_of(&served, "books").await.len(),
        1,
        "and the rest is left"
    );
}

// One deletion at a time: a second confirmed while the first is running waits
// its turn, is said to be waiting, and runs once the first has committed.
#[tokio::test]
async fn a_second_deletion_waits_for_the_first() {
    let served = Served::library().await;
    served.hold_storage();

    let (status, _) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202);
    // No sleep and no guess: the read is counted as it arrives, so the first
    // deletion is inside Storage when the second is asked for.
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
    let (status_armed, armed) =
        body_of(served.post("/api/delete?entry=albums/notes.txt").await).await;
    let (_, again) = body_of(served.post("/api/delete?entry=albums/notes.txt").await).await;
    served.release_storage();
    served.delete_idle().await;

    assert_eq!(status_armed, 202, "{armed}");
    assert_eq!(deletion(&armed)["run"], 1);
    assert_eq!(deletion(&armed)["status"], "deleting");
    assert_eq!(deletion(&armed)["waiting"], 1);
    assert_eq!(
        deletion(&again)["waiting"],
        1,
        "the same deletion asked for again is not queued twice"
    );

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(deletion(&work)["run"], 2);
    assert_eq!(deletion(&work)["status"], "done");
    assert_eq!(deletion(&work)["paths"], json!(["albums/notes.txt"]));
    assert_eq!(deletion(&work)["waiting"], 0);
    let left: Vec<String> = rows_of(&served, "albums")
        .await
        .into_iter()
        .map(|(name, _, _)| name)
        .collect();
    assert!(!left.contains(&"cover.png".to_owned()), "{left:?}");
    assert!(!left.contains(&"notes.txt".to_owned()), "{left:?}");
}

// Refused where a freeze is refused for the same reasons — a locked Library,
// and nothing current under the path — and refused for the Library root as a
// whole. Each before anything is armed.
#[tokio::test]
async fn a_deletion_is_refused_for_a_locked_library_the_root_and_nothing() {
    let served = Served::library().await;
    for method in ["GET", "POST"] {
        for root in ["/api/delete", "/api/delete?path="] {
            let (status, refusal) = crate::support::route(&served, method, root).await;
            assert_eq!(status, 400, "{method} {root}: {refusal}");
            assert_eq!(refusal["error"], "bad_path");
        }
        for nothing in [
            "/api/delete?path=nowhere",
            "/api/delete?entry=albums/gone.jpg",
            "/api/delete?entry=albums",
        ] {
            let (status, refusal) = crate::support::route(&served, method, nothing).await;
            assert_eq!(status, 404, "{method} {nothing}: {refusal}");
            assert_eq!(refusal["error"], "no_such_entry");
        }
    }
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["delete"], serde_json::Value::Null, "nothing was armed");

    served.lock();
    for method in ["GET", "POST"] {
        let (status, refusal) =
            crate::support::route(&served, method, "/api/delete?path=albums").await;
        assert_eq!(status, 423, "{method}: {refusal}");
        assert_eq!(refusal["error"], "locked");
    }
}

// A deletion planned over a Library another device changed meanwhile commits
// nothing (spec: CP-1, CP-18), and the explorer is told so as a conflict it can
// act on — run it again — rather than as a server that could not answer.
#[tokio::test]
async fn a_deletion_the_library_moved_underneath_stops_as_a_conflict() {
    let served = Served::library().await;
    served.hold_storage();

    let (status, _) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202);
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
    // The other device deletes the same file while this one is reading the
    // Library it planned from.
    served.delete_elsewhere("albums/cover.png").await;
    served.release_storage();
    served.delete_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    let stopped = deletion(&work);
    assert_eq!(stopped["status"], "stopped", "{work}");
    assert_eq!(stopped["stopped"]["error"], "conflict", "{work}");
    let said = stopped["stopped"]["message"].as_str().expect("a sentence");
    assert!(
        said.contains("another device changed the Library"),
        "{said}"
    );
    assert_eq!(stopped["entries"], 0, "nothing was committed");
}

// A deletion confirmed while a freeze is packing a book waits for it rather
// than failing: the two own this device's pending rows in turn (spec: OC-2),
// so a deletion that went ahead beside the freeze would be refused the rows
// and stop having committed nothing, with nothing a person could do
// but press Delete again later.
#[tokio::test]
async fn a_deletion_confirmed_while_a_book_is_packed_waits_for_it() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.hold_storage();

    served.arm_freeze("scans/vol-1");
    // No sleep and no guess: the freeze owns the pending rows before it reads
    // anything, so once a read is held it is packing.
    while served.held_reads() == 0 {
        tokio::task::yield_now().await;
    }
    let (status, armed) = body_of(served.post("/api/delete?entry=albums/cover.png").await).await;
    assert_eq!(status, 202, "{armed}");
    served.release_storage();
    served.freeze_idle().await;
    served.delete_idle().await;

    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["freeze"]["status"], "done", "{work}");
    let done = deletion(&work);
    assert_eq!(done["status"], "done", "{work}");
    assert_eq!(done["entries"], 1, "{work}");
}
