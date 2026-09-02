//! What the browser is answered with, over a Library another device filled.
//!
//! The router is driven as the service it is rather than through a socket, so
//! every case here is the whole path a request takes — the query, the device
//! layer, the catalog, and for one route the fetch that places a file — with
//! nothing standing in for any of it but the provider.

use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderName, Request};
use coffret_logging::testing::CapturedLogs;
use coffret_server::CAPABILITY_HEADER;
use serde_json::{json, Value};
use tracing::Level;

mod support;
use support::{asking, bytes, header, json as body_of, Served, SERVER_KEY};

/// The rows of one listing, as `(name, state, container, openable)`.
///
/// A row with no Container of its own is `""`: nothing has been committed for a
/// file somebody just added, so what it will live in is the next sync's answer.
fn files(listing: &serde_json::Value) -> Vec<(String, String, String, bool)> {
    listing["files"]
        .as_array()
        .expect("a listing carries files")
        .iter()
        .map(|file| {
            (
                file["name"].as_str().expect("a row has a name").to_owned(),
                file["state"]
                    .as_str()
                    .expect("a row has a state")
                    .to_owned(),
                file["container"].as_str().unwrap_or_default().to_owned(),
                file["openable"]
                    .as_bool()
                    .expect("a row says whether it can be opened"),
            )
        })
        .collect()
}

/// The rows of one listing, as `(name, state)`.
fn states(listing: &serde_json::Value) -> Vec<(String, String)> {
    files(listing)
        .into_iter()
        .map(|(name, state, ..)| (name, state))
        .collect()
}

/// What one folder holds, as the listing route answers it.
async fn listing_of(served: &Served, folder: &str) -> serde_json::Value {
    let (status, listing) = body_of(served.get(&format!("/api/list?path={folder}")).await).await;
    assert_eq!(status, 200, "the listing of {folder} answers");
    listing
}

/// The names of a listing's child folders.
fn folders(listing: &serde_json::Value) -> Vec<String> {
    listing["folders"]
        .as_array()
        .expect("a listing carries folders")
        .iter()
        .map(|folder| {
            folder["name"]
                .as_str()
                .expect("a folder has a name")
                .to_owned()
        })
        .collect()
}

/// Which of a listing's child folders this device has a folder for.
fn folders_mapped(listing: &serde_json::Value) -> Vec<(String, bool)> {
    listing["folders"]
        .as_array()
        .expect("a listing carries folders")
        .iter()
        .map(|folder| {
            (
                folder["name"]
                    .as_str()
                    .expect("a folder has a name")
                    .to_owned(),
                folder["mapped"]
                    .as_bool()
                    .expect("a folder row says whether this device has one for it"),
            )
        })
        .collect()
}

// One folder, one level down, in EP-3 order: the child folder and the child
// files, and nothing from inside the child folder.
#[tokio::test]
async fn a_folder_lists_its_child_folders_and_its_files() {
    let served = Served::library().await;

    let (status, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(status, 200);
    assert_eq!(listing["path"], "albums");
    assert_eq!(folders(&listing), ["2026"]);
    assert_eq!(
        files(&listing),
        [
            (
                "caf\u{e9}.jpg".to_owned(),
                "remote".to_owned(),
                "one-file".to_owned(),
                true
            ),
            (
                "cover.png".to_owned(),
                "remote".to_owned(),
                "one-file".to_owned(),
                true
            ),
            // A name a browser draws nothing from is a row like any other, and
            // one the explorer will not offer to open.
            (
                "notes.txt".to_owned(),
                "remote".to_owned(),
                "one-file".to_owned(),
                false
            ),
        ],
    );
    assert_eq!(
        listing["folders"][0]["path"], "albums/2026",
        "a child folder is named by its whole path",
    );

    let cover = &listing["files"][1];
    assert_eq!(cover["path"], "albums/cover.png");
    assert_eq!(cover["size"], 5);
    assert_eq!(cover["content_type"], "image/png");
    assert!(
        cover["mtime"]
            .as_str()
            .is_some_and(|mtime| mtime.ends_with('Z')),
        "a modification time is stated in UTC: {cover}",
    );
    assert_eq!(
        listing["files"][2]["content_type"],
        "application/octet-stream"
    );
}

// A request that names no folder is a request for the Library root, which is
// what an explorer's first one carries.
#[tokio::test]
async fn naming_no_folder_lists_the_library_root() {
    let served = Served::library().await;

    for uri in ["/api/list", "/api/list?path="] {
        let (status, listing) = body_of(served.get(uri).await).await;
        assert_eq!(status, 200, "{uri}");
        assert_eq!(listing["path"], "");
        assert_eq!(folders(&listing), ["albums", "books"], "{uri}");
        assert_eq!(files(&listing), [], "{uri}");
    }
}

// Flat and complete: every folder the separators imply, each named in full, for
// the browser to nest (spec: EP-2).
#[tokio::test]
async fn every_folder_of_the_library_is_listed_flat() {
    let served = Served::library().await;

    let (status, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] })
    );
}

// Three fields for the status bar, and nothing about where the Library lives
// beyond which provider it is on.
#[tokio::test]
async fn the_status_bar_is_told_which_library_this_is() {
    let served = Served::library().await;

    let (status, library) = body_of(served.get("/api/library").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        library,
        json!({
            "name": "served",
            "library_id": "1111111111111111",
            "provider": "s3",
        }),
    );
}

// EP-10: an Entry this device never materialized is remote, and asking for its
// bytes is what makes it present — the file is placed in the mapped folder and
// the row says so from then on.
#[tokio::test]
async fn asking_for_a_remote_entry_fetches_it_and_makes_it_present() {
    let served = Served::library().await;
    assert!(!served.holds("albums/2026/spring.jpg"));

    let answer = served.get("/api/file?path=albums/2026/spring.jpg").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(header(&answer, "content-type"), "image/jpeg");
    // The user's own plaintext: no shared cache keeps it and no browser writes
    // it to disk.
    assert_eq!(header(&answer, "cache-control"), "private, no-store");
    assert_eq!(bytes(answer).await, b"spring");
    assert!(served.holds("albums/2026/spring.jpg"));
}

// A reader that opens a page and prefetches it, two tabs on one folder, or the
// background fill and a click landing on one Entry: every one of them answers
// with the Entry, and the Container is read once (spec: PK-16).
//
// What one fetch costs is measured rather than assumed — a range read of one
// Entry is several reads of one object, and how many is the fetch's business
// and not this case's. So the case places one Entry on its own first, in a
// folder holding nothing else for the fill to go on with, and everything after
// that is counted in multiples of what it cost: the Containers are the same
// shape, so anything that ran the flow twice for one Entry would show up as
// double.
#[tokio::test]
async fn one_entry_asked_for_twice_at_once_is_fetched_once() {
    let served = Served::library().await;

    let alone = served.get("/api/file?path=books/page-001.png").await;
    assert_eq!(alone.status(), 200);
    served.fill_settled().await;
    let once = served.ranged_reads();
    assert!(once > 0, "fetching an Entry reads part of its Container");

    // Two callers on one Entry, and the fill they arm going after the other
    // Entry of that folder at the same time. Three Containers are read in all,
    // once each.
    let (first, second) = served
        .get_twice("/api/file?path=albums/2026/summer.jpg")
        .await;
    assert_eq!(first.status(), 200);
    assert_eq!(second.status(), 200);
    assert_eq!(bytes(first).await, b"summer");
    assert_eq!(bytes(second).await, b"summer");
    served.fill_settled().await;
    assert!(
        served.holds("albums/2026/spring.jpg"),
        "the fill brought the rest of the folder over"
    );
    assert_eq!(
        served.ranged_reads(),
        once * 3,
        "everyone after the first waits for its verdict rather than reading again",
    );
}

// A name a browser draws nothing from is served as bytes with no claim about
// them, rather than refused.
#[tokio::test]
async fn a_file_no_browser_draws_is_served_as_bytes() {
    let served = Served::library().await;

    let answer = served.get("/api/file?path=albums/notes.txt").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(header(&answer, "content-type"), "application/octet-stream");
    assert_eq!(bytes(answer).await, b"a note about the albums");
}

// EP-1: a name arrives in whichever spelling the caller's platform keeps, and
// becomes the Library's on the way in. `caf%C3%A9` and `cafe%CC%81` are the two
// spellings of one file, and both name the Entry that is there — refusing the
// second would be telling somebody their own filename is malformed.
#[tokio::test]
async fn a_name_in_another_spelling_names_the_same_entry() {
    let served = Served::library().await;

    for uri in [
        "/api/file?path=albums/caf%C3%A9.jpg",
        "/api/file?path=albums/cafe%CC%81.jpg",
    ] {
        let answer = served.get(uri).await;
        assert_eq!(answer.status(), 200, "{uri}");
        assert_eq!(bytes(answer).await, b"a cafe", "{uri}");
    }
}

// EP-5: the Library holds at most one current Entry at a path, and none there is
// an answer about the request rather than a failure.
#[tokio::test]
async fn a_path_the_library_holds_nothing_at_is_not_found() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.get("/api/file?path=albums/nothing.jpg").await).await;
    assert_eq!(status, 404);
    assert_eq!(refusal["error"], "no_such_entry");
}

// EP-2: a path that is not one is refused before anything is asked of the
// Library, and the refusal says which rule it broke.
#[tokio::test]
async fn a_path_that_is_not_an_entry_path_is_refused() {
    let served = Served::library().await;

    for uri in [
        "/api/file?path=albums/../../etc/passwd",
        "/api/file?path=/albums/cover.png",
        "/api/file?path=",
        "/api/list?path=albums//2026",
    ] {
        let (status, refusal) = body_of(served.get(uri).await).await;
        assert_eq!(status, 400, "{uri}");
        assert_eq!(refusal["error"], "bad_path", "{uri}");
        assert!(
            refusal["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty()),
            "the refusal says which rule went: {refusal}",
        );
    }
}

// EP-9: a mapping is what makes a local path exist at all, so an Entry outside
// every one of them is a fact about this device — which the explorer shows as
// "this folder is not on this device".
#[tokio::test]
async fn an_entry_no_folder_on_this_device_holds_is_declined() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.get("/api/file?path=books/page-001.png").await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "unmapped");

    // The Library still holds it, and the catalog still lists it: what is
    // missing is a folder here to put it in.
    let (status, listing) = body_of(served.get("/api/list?path=books").await).await;
    assert_eq!(status, 200);
    assert_eq!(folders(&listing), Vec::<String>::new());
    assert_eq!(files(&listing).len(), 1);
    assert_eq!(
        listing["mapped"], false,
        "the listing said so before anything was clicked",
    );
}

// EP-9: the mappings answer "is this folder on this device" out of the catalog,
// so the listing carries it and a browser never has to find out by being
// declined. The children of the Library root are where two siblings differ,
// since a mapping is made at the top level.
#[tokio::test]
async fn a_listing_says_which_folders_this_device_has_one_for() {
    let served = Served::mapping_only("albums").await;

    let (status, root) = body_of(served.get("/api/list").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        root["mapped"], false,
        "a top-level mapping stands for its own subtree and not for what sits beside it",
    );
    assert_eq!(
        folders_mapped(&root),
        [("albums".to_owned(), true), ("books".to_owned(), false)],
    );

    let (_, albums) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(albums["mapped"], true);
    assert_eq!(folders_mapped(&albums), [("2026".to_owned(), true)]);
}

// A mapping at the Library root represents everything the top-level ones do
// not, so with one present every folder is on this device (spec: EP-9).
#[tokio::test]
async fn a_device_that_maps_the_library_root_has_a_folder_for_everything() {
    let served = Served::library().await;

    for uri in ["/api/list", "/api/list?path=albums", "/api/list?path=books"] {
        let (status, listing) = body_of(served.get(uri).await).await;
        assert_eq!(status, 200, "{uri}");
        assert_eq!(listing["mapped"], true, "{uri}");
        assert!(
            folders_mapped(&listing).iter().all(|(_, mapped)| *mapped),
            "{uri}: {listing}",
        );
    }
}

// EP-11: a fetch places a file only where this device can vouch for what is
// there, and a file it did not put there may be content the Library has never
// held. Left byte-for-byte as it is, and the browser told what was found.
#[tokio::test]
async fn a_file_this_device_did_not_place_is_never_overwritten() {
    let served = Served::library().await;
    served.plant_locally("albums/cover.png", b"something of my own");

    let (status, refusal) = body_of(served.get("/api/file?path=albums/cover.png").await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "surfaced");
    assert_eq!(refusal["surfaced"], "ForeignFile");
    assert_eq!(
        std::fs::read(served.local_path("albums/cover.png")).expect("the file is still there"),
        b"something of my own",
    );
}

/// The fill the server is on, or `null`.
fn fill(activity: &serde_json::Value) -> &serde_json::Value {
    &activity["fill"]
}

/// The Entries one fill declined, as `(path, reason)`.
fn declined(fill: &serde_json::Value) -> Vec<(String, String)> {
    fill["declined"]
        .as_array()
        .expect("a fill says what it declined")
        .iter()
        .map(|entry| {
            (
                entry["path"]
                    .as_str()
                    .expect("a declined Entry has a path")
                    .to_owned(),
                entry["reason"]
                    .as_str()
                    .expect("a declined Entry says which way it was declined")
                    .to_owned(),
            )
        })
        .collect()
}

// An explorer that has opened nothing and dropped nothing has nothing to be told
// about, and the route says so rather than inventing work nobody started.
#[tokio::test]
async fn nothing_is_happening_before_anything_is_opened_or_dropped() {
    let served = Served::library().await;

    let (status, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        activity,
        json!({ "fill": null, "sync": null, "freeze": null })
    );
}

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
    let (_, armed) = body_of(served.get("/api/activity").await).await;
    assert_eq!(fill(&armed)["folder"], "albums");

    served.fill_settled().await;
    let (_, done) = body_of(served.get("/api/activity").await).await;
    assert_eq!(fill(&done)["folder"], "albums");
    assert_eq!(fill(&done)["status"], "done");
    assert_eq!(
        (fill(&done)["done"].as_u64(), fill(&done)["total"].as_u64()),
        (Some(2), Some(2)),
        "the two rows the listing still called remote, and no others: {done}",
    );
    assert_eq!(declined(fill(&done)), Vec::<(String, String)>::new());
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
    served.fill_settled().await;

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    let fill = fill(&activity);
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
    served.fill_settled().await;

    let (_, stopped) = body_of(served.get("/api/activity").await).await;
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
    served.fill_settled().await;

    let (_, finished) = body_of(served.get("/api/activity").await).await;
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
    served.fill_settled().await;

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(fill(&activity)["folder"], "books");
    assert_eq!(fill(&activity)["status"], "done");
    assert!(served.holds("books/page-001.png"));

    let (_, left) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert!(
        states(&left).iter().all(|(_, state)| state == "remote"),
        "the folder that was left is not resumed on its own: {left}",
    );
}

// A folder no mapping of this device reaches has nowhere to put a file
// (spec: EP-9), so there is nothing there to bring over — and the fill says so
// rather than asking Storage once per file to be told so once per file.
#[tokio::test]
async fn a_fill_of_a_folder_this_device_has_no_room_for_does_nothing() {
    let served = Served::mapping_only("albums").await;

    assert_eq!(served.post("/api/fill?path=books").await.status(), 202);
    served.fill_settled().await;

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(fill(&activity)["folder"], "books");
    assert_eq!(fill(&activity)["status"], "done");
    assert_eq!(fill(&activity)["total"], 0);
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

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(
        activity,
        json!({ "fill": null, "sync": null, "freeze": null })
    );
}

/// What the activity says about the sync, which every drop arms.
fn sync(activity: &serde_json::Value) -> &serde_json::Value {
    let sync = &activity["sync"];
    assert!(!sync.is_null(), "a sync has been armed: {activity}");
    sync
}

/// The names a drop wrote, as Entry Paths.
fn written(answer: &serde_json::Value) -> Vec<String> {
    answer["written"]
        .as_array()
        .expect("a drop says what it wrote")
        .iter()
        .map(|path| path.as_str().expect("an Entry Path").to_owned())
        .collect()
}

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

    served.sync_settled().await;
    let (_, listing) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert_eq!(
        states(&listing),
        [
            ("held.jpg".to_owned(), "uploading".to_owned()),
            ("moor.jpg".to_owned(), "uploading".to_owned()),
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
    served.sync_settled().await;

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

// A sync that Storage stopped is reported the way a fill that Storage stopped is
// — the state the retry is offered from — and the retry finishes once the store
// is back, with the files still sitting in the folder where the drop left them.
#[tokio::test]
async fn a_sync_storage_stopped_is_reported_and_finishes_when_the_store_comes_back() {
    let served = Served::library().await;
    served.halt_storage();

    served.upload("albums", &[("late.jpg", b"late")]).await;
    served.sync_settled().await;

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(sync(&activity)["status"], "stopped");
    assert_eq!(sync(&activity)["stopped"]["error"], "storage");
    assert_eq!(sync(&activity)["added"], 0);

    served.resume_storage();
    let (status, armed) = body_of(served.post("/api/sync").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        sync(&armed)["status"],
        "syncing",
        "the failure it is retrying is off the screen the moment the retry is armed",
    );

    served.sync_settled().await;
    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(sync(&activity)["status"], "done");
    assert_eq!(sync(&activity)["added"], 1);
    assert_eq!(sync(&activity)["stopped"], serde_json::Value::Null);
    assert_eq!(sync(&activity)["noted"], json!([]));
}

// EP-9: a folder no mapping of this device reaches has nowhere to put a single
// one of the files, so the whole drop is refused at once rather than once per
// file — the same verdict the listing already shows over the rows.
#[tokio::test]
async fn a_drop_onto_a_folder_that_is_not_on_this_device_is_refused_whole() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.upload("books", &[("new.png", b"new")]).await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "unmapped");

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(
        activity["sync"],
        serde_json::Value::Null,
        "nothing landed, so there is nothing to carry in",
    );
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
            "error": "declined",
            "reason": "pack_resident",
            "message": "the Library holds this file inside a Pack, and coffret cannot replace \
                        one of those yet",
        }]),
    );
    assert!(
        !served.holds("books/page-001.png"),
        "the refusal was settled before any byte was written",
    );
    assert!(served.holds("books/page-002.png"));

    // The sibling landed, so a sync was armed: it is waited out here rather than
    // left running past the end of the case, over folders the case is about to
    // remove.
    served.sync_settled().await;
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
    served.sync_settled().await;
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

// The first thing a server does, and the whole of what makes a joined device
// worth serving: the Journal is replayed into the catalog, and the Library is on
// the screen without anything having been typed at a terminal (spec: CK-9).
//
// And nothing else happens. No Container is read and no file is placed: every
// row arrives `remote`, which is exactly what a device that has the catalog and
// none of the files is (spec: EP-10).
#[tokio::test]
async fn starting_up_catches_the_catalog_up_with_the_library() {
    let served = Served::joined().await;

    let (status, before) = body_of(served.get("/api/folders").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        before,
        json!({ "folders": [] }),
        "a device that has just joined has replayed nothing",
    );

    served.start_up().await;

    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] })
    );
    let (_, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert!(
        states(&listing).iter().all(|(_, state)| state == "remote"),
        "the catalog knows them and this device has none of them: {listing}",
    );
    assert_eq!(
        served.ranged_reads(),
        0,
        "a catch-up opens no Container: a record carries what the ones it adds hold",
    );
    assert!(!served.holds("albums/cover.png"), "and places no file");
}

// Browsing the Index needs no Storage, so a Storage that is away is not a server
// that refuses to start: what it holds is served, and the refresh is what asks
// again once there is a bucket to ask.
#[tokio::test]
async fn a_startup_that_cannot_reach_storage_still_serves_what_the_index_holds() {
    let served = Served::joined().await;
    served.halt_storage();

    served.start_up().await;
    assert!(
        served.refused_reads() > 0,
        "the startup did reach for the Library's head",
    );

    let (status, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(status, 200, "the server answers rather than being dead");
    assert_eq!(
        listed,
        json!({ "folders": [] }),
        "and answers with what this device knows, which is nothing yet",
    );

    served.resume_storage();
    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["advanced"], true);

    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] }),
        "the retry is the whole recovery: no restart, no terminal",
    );
}

// The other way Storage goes away, and the one that would otherwise never end:
// it takes the read and answers neither way, which is what a filtered network
// looks like from here. The catch-up runs before the socket is bound, so without
// a deadline of its own the explorer would not be unreachable for a while — it
// would be unreachable for as long as the silence lasted.
//
// The clock is the case's own: `start_paused` moves it forward whenever nothing
// is runnable, so the deadline is reached at once and nothing here waits a real
// minute. The outer timeout is a generous multiple of it, and is what fails this
// case rather than hanging it if the deadline is ever taken back out.
#[tokio::test(start_paused = true)]
async fn a_startup_that_storage_never_answers_gives_up_and_serves() {
    let served = Served::joined().await;
    served.stall_storage();

    let started = tokio::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(600), served.start_up())
        .await
        .expect("the startup gives up on its own rather than waiting on Storage forever");
    assert!(
        served.stalled_reads() > 0,
        "the startup did reach for the Library's head",
    );
    assert!(
        started.elapsed() >= Duration::from_secs(30),
        "and waited on it, rather than never having gone out at all: {:?}",
        started.elapsed(),
    );

    let (status, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(status, 200, "the server answers rather than being dead");
    assert_eq!(
        listed,
        json!({ "folders": [] }),
        "and answers with what this device knows, which is nothing yet",
    );

    // What was abandoned leaves nothing behind that stops the next run: a
    // catch-up is replayed a record at a time, and the catalog's checkpoint is
    // where the one after it carries on from (spec: CK-9).
    served.resume_storage();
    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["advanced"], true);

    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books"] }),
        "the retry is the whole recovery here too: no restart, no terminal",
    );
}

// What the refresh is for. Another device commits while this server is up, and
// pressing refresh is how the person looking at the folder finds out — the row
// appears, `remote`, and the bytes stay where they are until it is opened.
#[tokio::test]
async fn a_refresh_brings_in_what_another_device_committed() {
    let served = Served::library().await;
    assert_eq!(files(&listing_of(&served, "albums").await).len(), 3);

    served.commit_elsewhere("albums/late.jpg", b"late").await;
    assert_eq!(
        files(&listing_of(&served, "albums").await).len(),
        3,
        "nothing has told this device about it yet",
    );

    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        (
            refreshed["advanced"].as_bool(),
            refreshed["gained"].as_i64(),
            refreshed["entries"].as_u64(),
        ),
        (Some(true), Some(1), Some(7)),
        "one Entry more than the six the fixture planted: {refreshed}",
    );

    let listing = listing_of(&served, "albums").await;
    assert_eq!(
        states(&listing),
        [
            ("caf\u{e9}.jpg".to_owned(), "remote".to_owned()),
            ("cover.png".to_owned(), "remote".to_owned()),
            ("late.jpg".to_owned(), "remote".to_owned()),
            ("notes.txt".to_owned(), "remote".to_owned()),
        ],
        "the new row is the Library's and not this device's: {listing}",
    );
    assert!(
        !served.holds("albums/late.jpg"),
        "a refresh brings over the catalog and not the bytes",
    );
}

// The ordinary answer, and the one a person presses refresh for most often: the
// catalog is where the Library is, and the screen says so rather than saying
// nothing.
#[tokio::test]
async fn a_refresh_with_nothing_new_says_the_catalog_is_up_to_date() {
    let served = Served::library().await;

    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(
        (
            refreshed["advanced"].as_bool(),
            refreshed["gained"].as_i64(),
            refreshed["entries"].as_u64(),
        ),
        (Some(false), Some(0), Some(6)),
        "the head it stands at is the head there is: {refreshed}",
    );
}

// A Storage that has gone is what a refresh most often meets, and it is offered
// again rather than reported as the server failing: the catalog stands where it
// stood, every other route goes on answering out of it, and the next press is
// the whole of the recovery.
#[tokio::test]
async fn a_refresh_storage_cannot_answer_is_a_retryable_refusal() {
    let served = Served::library().await;
    served.commit_elsewhere("albums/late.jpg", b"late").await;
    served.halt_storage();

    let (status, refusal) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 502);
    assert_eq!(refusal["error"], "storage");
    assert!(
        refusal["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "the refusal says something a person could act on: {refusal}",
    );

    let (status, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(status, 200, "the rest of the explorer is untouched");
    assert_eq!(files(&listing).len(), 3);

    served.resume_storage();
    let (status, refreshed) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
    assert_eq!(refreshed["gained"], 1);
    assert_eq!(files(&listing_of(&served, "albums").await).len(), 4);
}

// Two presses at once — a double click, two tabs — replay one after the other
// rather than both at once, and both are answered. The second finds the head the
// first one reached, which is the ordinary "nothing new".
#[tokio::test]
async fn two_refreshes_at_once_replay_one_after_the_other() {
    let served = Served::library().await;
    served.commit_elsewhere("albums/late.jpg", b"late").await;

    let (first, second) = tokio::join!(served.post("/api/refresh"), served.post("/api/refresh"));
    let (first_status, first_body) = body_of(first).await;
    let (second_status, second_body) = body_of(second).await;
    assert_eq!(first_status, 200);
    assert_eq!(second_status, 200);

    let gained = [&first_body, &second_body]
        .map(|answered| answered["gained"].as_i64().expect("a count"))
        .iter()
        .sum::<i64>();
    assert_eq!(
        gained, 1,
        "the Entry is counted once: {first_body}, {second_body}",
    );
    assert_eq!(files(&listing_of(&served, "albums").await).len(), 4);
}

// A file whose Entry left the Library — another device removed the Container
// holding it — is the same state as one just dropped, and is shown the same way:
// this device has it and the Library does not.
#[tokio::test]
async fn a_file_the_library_no_longer_holds_is_shown_as_this_devices_own() {
    let served = Served::library().await;
    served.plant_locally("albums/theirs.jpg", b"not in the Library");

    let (_, listing) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(
        states(&listing),
        [
            ("caf\u{e9}.jpg".to_owned(), "remote".to_owned()),
            ("cover.png".to_owned(), "remote".to_owned()),
            ("notes.txt".to_owned(), "remote".to_owned()),
            ("theirs.jpg".to_owned(), "uploading".to_owned()),
        ],
    );
}

/// What the activity says about the freeze, which a book drop arms.
fn freeze(activity: &serde_json::Value) -> &serde_json::Value {
    let freeze = &activity["freeze"];
    assert!(!freeze.is_null(), "a freeze has been armed: {activity}");
    freeze
}

/// Every row of `folder`, as `(name, state, container)`.
async fn rows_of(served: &Served, folder: &str) -> Vec<(String, String, String)> {
    files(&listing_of(served, folder).await)
        .into_iter()
        .map(|(name, state, container, _)| (name, state, container))
        .collect()
}

/// The three pages of the book the freeze cases bring in.
const BOOK: [(&str, &[u8]); 3] = [
    ("page-001.jpg", b"the first page"),
    ("page-002.jpg", b"the second page"),
    ("page-003.jpg", b"the third page"),
];

/// The whole of what a book drop is for. Three pages land in a folder somebody
/// made in the browser, the freeze the drop armed packs them, and what the
/// Library ends up holding is Packs rather than one Container per page
/// (spec: PK-1, PK-7, PK-17).
///
/// And the sync is not armed. That is not a detail: a sync over these files
/// would carry each of them in as a Container of its own, which is the shape the
/// freeze exists to avoid — so a drop that armed both would pack a book that had
/// already been committed one page at a time.
#[tokio::test]
async fn a_book_dropped_into_a_new_folder_is_packed_rather_than_synced() {
    let served = Served::library().await;

    let (status, answer) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200);
    assert_eq!(
        written(&answer),
        [
            "scans/vol-1/page-001.jpg",
            "scans/vol-1/page-002.jpg",
            "scans/vol-1/page-003.jpg",
        ],
    );
    assert_eq!(answer["refused"], json!([]));

    served.freeze_settled().await;
    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(freeze(&activity)["folder"], "scans/vol-1");
    assert_eq!(freeze(&activity)["status"], "done");
    assert_eq!(freeze(&activity)["packs"], 1);
    assert_eq!(freeze(&activity)["entries"], 3);
    assert_eq!(freeze(&activity)["noted"], json!([]));
    assert_eq!(freeze(&activity)["stopped"], serde_json::Value::Null);
    assert_eq!(
        activity["sync"],
        serde_json::Value::Null,
        "a book drop arms the freeze and nothing else: {activity}",
    );

    assert_eq!(
        rows_of(&served, "scans/vol-1").await,
        BOOK.map(|(name, _)| (name.to_owned(), "present".to_owned(), "pack".to_owned())),
        "every page is an Entry this device has, and all of them live in Packs",
    );
    // And the folder is in the Library rather than only on this device: a folder
    // is what the separators of its Entries' paths imply (spec: EP-2), so the
    // tree draws it only once something under it is committed.
    let (_, listed) = body_of(served.get("/api/folders").await).await;
    assert_eq!(
        listed,
        json!({ "folders": ["albums", "albums/2026", "books", "scans", "scans/vol-1"] }),
    );
}

// EP-9: a folder no mapping of this device reaches has nowhere to put a single
// page, so the whole drop is refused at once — before any byte, and whichever
// gesture it was.
#[tokio::test]
async fn a_book_dropped_where_this_device_has_no_folder_is_refused_whole() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.upload_book("books/vol-1", &BOOK).await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "unmapped");

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(
        activity,
        json!({ "fill": null, "sync": null, "freeze": null }),
        "nothing landed, so there is nothing to pack",
    );
}

// PK-10, PK-12: the per-part rules are the drop's, not the sync's, so a book
// drop meets them identically. A page whose name the Library already holds
// inside a Pack is refused by name — coffret cannot replace one yet — and the
// page beside it lands and is packed.
#[tokio::test]
async fn a_page_the_library_holds_inside_a_pack_is_refused_and_the_rest_is_packed() {
    let served = Served::packed_library().await;

    let (status, answer) = body_of(
        served
            .upload_book(
                "books",
                &[("page-001.png", b"mine"), ("page-002.png", b"next")],
            )
            .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer), ["books/page-002.png"]);
    assert_eq!(answer["refused"][0]["name"], "page-001.png");
    assert_eq!(answer["refused"][0]["reason"], "pack_resident");
    assert!(
        !served.holds("books/page-001.png"),
        "the refusal was settled before any byte was written",
    );

    served.freeze_settled().await;
    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(freeze(&activity)["status"], "done");
    assert_eq!(freeze(&activity)["entries"], 1);
    assert_eq!(
        rows_of(&served, "books").await,
        [
            (
                "page-001.png".to_owned(),
                "remote".to_owned(),
                "pack".to_owned()
            ),
            (
                "page-002.png".to_owned(),
                "present".to_owned(),
                "pack".to_owned()
            ),
        ],
        "the page that landed was packed, and the Pack beside it was left alone",
    );
}

// One book at a time. A second folder asked for while one is being packed waits
// its turn rather than taking its place: a freeze commits one batch (spec:
// PK-7), so one abandoned half way brings in no part of its book — where a
// fill, which does follow whoever is clicking, leaves behind exactly the files
// it had already brought over.
//
// Two armings with nothing awaited between them, which is the one way to say
// this as a case: anything awaited would let the worker finish the first.
#[tokio::test]
async fn a_second_book_waits_for_the_first_rather_than_taking_its_place() {
    let served = Served::library().await;
    served.plant_locally("scans/vol-1/page-001.jpg", b"the first book");
    served.plant_locally("scans/vol-2/page-001.jpg", b"the second book");

    served.arm_freeze("scans/vol-1");
    served.arm_freeze("scans/vol-2");
    served.freeze_settled().await;

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(freeze(&activity)["folder"], "scans/vol-2");
    assert_eq!(freeze(&activity)["status"], "done");
    assert_eq!(
        freeze(&activity)["entries"],
        1,
        "the run on record is the second book's own, not one sweep of both",
    );

    for folder in ["scans/vol-1", "scans/vol-2"] {
        assert_eq!(
            rows_of(&served, folder).await,
            [(
                "page-001.jpg".to_owned(),
                "present".to_owned(),
                "pack".to_owned()
            )],
            "{folder} was packed rather than left where the other one displaced it",
        );
    }
}

// A Storage that has gone stops a freeze the way it stops a fill and a sync, and
// it is reported the same way: the pages stay in the folder, the Library is
// untouched, and `POST /api/freeze` is the whole of the recovery — no restart
// and no dropping the book again.
#[tokio::test]
async fn storage_stops_a_freeze_and_the_book_can_be_packed_again() {
    let served = Served::library().await;
    served.halt_storage();

    let (status, answer) = body_of(served.upload_book("scans/vol-1", &BOOK).await).await;
    assert_eq!(status, 200);
    assert_eq!(written(&answer).len(), 3);
    served.freeze_settled().await;

    let (_, stopped) = body_of(served.get("/api/activity").await).await;
    assert_eq!(freeze(&stopped)["folder"], "scans/vol-1");
    assert_eq!(freeze(&stopped)["status"], "stopped");
    assert_eq!(freeze(&stopped)["stopped"]["error"], "storage");
    assert_eq!(freeze(&stopped)["packs"], 0);
    assert!(
        rows_of(&served, "scans/vol-1")
            .await
            .iter()
            .all(|(_, state, container)| state == "uploading" && container.is_empty()),
        "the pages are in the folder and the Library holds none of them",
    );

    served.resume_storage();
    let (status, armed) = body_of(served.post("/api/freeze?path=scans/vol-1").await).await;
    assert_eq!(status, 202);
    assert_eq!(
        freeze(&armed)["status"],
        "freezing",
        "the failure it is retrying is off the screen the moment the retry is armed",
    );

    served.freeze_settled().await;
    let (_, finished) = body_of(served.get("/api/activity").await).await;
    assert_eq!(freeze(&finished)["status"], "done");
    assert_eq!(freeze(&finished)["entries"], 3);
    assert_eq!(freeze(&finished)["stopped"], serde_json::Value::Null);
    assert_eq!(
        rows_of(&served, "scans/vol-1").await,
        BOOK.map(|(name, _)| (name.to_owned(), "present".to_owned(), "pack".to_owned())),
    );
}

// EP-9: a freeze of a folder no mapping reaches would walk to select nothing and
// commit nothing, so it is refused rather than armed — a `202` for work that
// cannot happen is a browser told to follow a run that will never say anything.
#[tokio::test]
async fn a_freeze_of_a_folder_this_device_has_no_room_for_is_refused() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.post("/api/freeze?path=books").await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["error"], "declined");
    assert_eq!(refusal["reason"], "unmapped");

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(activity["freeze"], serde_json::Value::Null);
}

// EP-2: the folder a freeze is asked for is held to the same shape every other
// path on these routes is, and refused before anything is armed.
#[tokio::test]
async fn a_freeze_of_something_that_is_not_a_folder_is_refused() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.post("/api/freeze?path=albums/../etc").await).await;
    assert_eq!(status, 400);
    assert_eq!(refusal["error"], "bad_path");

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(activity["freeze"], serde_json::Value::Null);
}

// PK-17: a freeze is of one folder, and a prefix narrowed to nothing selects
// every eligible Entry the mappings reach. An absent `?path=` is the Library
// root everywhere else on these routes, so leaving it out here would be the
// command line's whole-Library run arrived at by omission — and this fixture
// maps the Library root, which is the device on which nothing else stands
// between the two.
#[tokio::test]
async fn a_freeze_that_names_no_folder_is_refused() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.post("/api/freeze").await).await;
    assert_eq!(status, 400);
    assert_eq!(refusal["error"], "bad_path");

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(activity["freeze"], serde_json::Value::Null);
}

// The same rule reached through the drop, which is how a browser reaches this at
// all: a book is brought into the folder made for it, so `freeze=true` naming no
// folder is refused before a byte is read rather than packing the whole Library
// around the pages that were dropped.
#[tokio::test]
async fn a_book_dropped_onto_the_library_root_is_refused_whole() {
    let served = Served::library().await;

    let (status, refusal) = body_of(served.upload_book("", &BOOK).await).await;
    assert_eq!(status, 400);
    assert_eq!(refusal["error"], "bad_path");
    assert!(
        !served.holds("page-001.jpg"),
        "the refusal was settled before any byte was written",
    );

    let (_, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(
        activity,
        json!({ "fill": null, "sync": null, "freeze": null }),
        "nothing landed, and nothing was armed",
    );
}

// ---------------------------------------------------------------------------
// Who is answered at all.
//
// The cases above are about what a route says to the explorer on this device.
// These are about the fences in front of every one of them, which are what makes
// "the explorer on this device" mean anything.
// ---------------------------------------------------------------------------

/// Every route this server has, as a caller reaches it.
///
/// Written out rather than derived, because what the case over it says is that
/// there is no route without the fences — and a list the router generated would
/// hold whatever the router holds.
const EVERY_ROUTE: [(&str, &str); 12] = [
    ("GET", "/api/library"),
    ("GET", "/api/folders"),
    ("GET", "/api/list"),
    ("GET", "/api/file?path=albums/cover.png"),
    ("GET", "/api/activity"),
    ("POST", "/api/fill?path=albums"),
    // The lock is behind the fence like everything else, and needs to be:
    // shutting somebody's Library is a thing done to it, and a page on another
    // site could otherwise shut one it may not even read.
    ("POST", "/api/lock"),
    ("POST", "/api/sync"),
    ("POST", "/api/freeze?path=albums"),
    ("POST", "/api/refresh"),
    ("POST", "/api/upload?path=albums"),
    ("POST", "/api/upload?path=albums&freeze=true"),
];

/// The same request the explorer would send, with one header put otherwise.
///
/// Replaced rather than added to, and removed where the case says nothing:
/// [`asking`] has already put the address and the key on the request, and a
/// second value under one name would leave the first one standing.
fn instead(header: &str, value: Option<&str>, method: &str, uri: &str) -> Request<Body> {
    let mut builder = asking(method, uri);
    let headers = builder
        .headers_mut()
        .expect("the request is well formed so far");
    let name = HeaderName::from_bytes(header.as_bytes()).expect("a case names real headers");
    match value {
        Some(value) => {
            headers.insert(name, value.parse().expect("a case sends text"));
        }
        None => {
            headers.remove(name);
        }
    }
    builder
        .body(Body::empty())
        .expect("a request with no body is well formed")
}

// Reads and mutations alike, and no exception for the ones that only say what
// the server knows: an Entry Path is the person's own name for their file, and
// the file route answers with plaintext.
#[tokio::test]
async fn no_route_answers_a_request_that_shows_no_key() {
    let served = Served::library().await;

    for (method, uri) in EVERY_ROUTE {
        let asked = instead(CAPABILITY_HEADER, None, method, uri);
        let (status, refusal) = body_of(served.send(asked).await).await;
        assert_eq!(status, 403, "{method} {uri} answered without a key");
        assert_eq!(refusal["error"], "unauthorized", "{method} {uri}");
    }
}

// A key of the right shape and the wrong value, on a read and on a mutation.
// Answered exactly as no key at all, so that a caller guessing learns nothing
// from the difference between the two.
#[tokio::test]
async fn a_key_that_is_not_this_server_s_is_refused() {
    let served = Served::library().await;
    let wrong = "0000000000000000000000000000000000000000000000000000000000000000";

    for (method, uri) in [("GET", "/api/list?path=albums"), ("POST", "/api/refresh")] {
        let asked = instead(CAPABILITY_HEADER, Some(wrong), method, uri);
        let (status, refusal) = body_of(served.send(asked).await).await;
        assert_eq!(status, 403, "{method} {uri} answered a key it never drew");
        assert_eq!(refusal["error"], "unauthorized", "{method} {uri}");
    }

    // And the same two, shown the key this server did draw.
    let (status, _) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(status, 200);
    let (status, _) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
}

// A hostname somebody else's DNS pointed at this socket arrives carrying that
// name, and nothing about the connection tells it from the explorer's request.
// The `Host` does, and it is read before the key: a request that reached here by
// another name is refused whatever else it carries.
#[tokio::test]
async fn a_host_naming_somewhere_else_is_refused_holding_the_key() {
    let served = Served::library().await;

    let asked = instead(
        "host",
        Some("coffret.example.com:8787"),
        "GET",
        "/api/list?path=albums",
    );

    let (status, refusal) = body_of(served.send(asked).await).await;
    assert_eq!(status, 403);
    assert_eq!(refusal["error"], "unauthorized");
}

// The second fence. `Origin` and `Sec-Fetch-Site` are the browser's own account
// of where a request came from and a page cannot forge either, so a page on
// another site is refused even in the state where it somehow holds a key.
#[tokio::test]
async fn a_page_on_another_site_is_refused_holding_the_key() {
    let served = Served::library().await;

    for (header, value) in [
        ("origin", "https://elsewhere.example"),
        ("sec-fetch-site", "cross-site"),
    ] {
        let asked = instead(header, Some(value), "POST", "/api/sync");
        let (status, refusal) = body_of(served.send(asked).await).await;
        assert_eq!(status, 403, "a {header} of {value} was answered");
        assert_eq!(refusal["error"], "unauthorized", "{header}: {value}");
    }
}

// The one thing a refusal must not do is help. Neither the body nor the log says
// what the key is — the body is displayed verbatim on a screen and the log is
// read somewhere the request never was — and neither says what was shown either,
// which is a caller's own text.
#[tokio::test]
async fn a_refusal_never_says_what_the_key_is() {
    let served = Served::library().await;
    let logs = CapturedLogs::capture();

    let guessed = "8f14e45fceea167a5a36dedd4bea2543a1b2c3d4e5f60718293a4b5c6d7e8f91";
    let asked = instead(
        CAPABILITY_HEADER,
        Some(guessed),
        "GET",
        "/api/list?path=albums",
    );

    let (status, refusal) = body_of(served.send(asked).await).await;
    assert_eq!(status, 403);
    let said = refusal.to_string();
    assert!(
        !said.contains(SERVER_KEY),
        "the refusal echoed the key: {said}"
    );
    assert!(
        !said.contains(guessed),
        "the refusal echoed the guess: {said}"
    );

    // Something was written down — a refusal nobody could find out about would
    // be worse than one that said too much — and it names the fence and nothing
    // the request carried.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "admit");
    assert_eq!(event.field("refused"), "key");
    logs.assert_free_of(&[SERVER_KEY, guessed]);
}

// ---------------------------------------------------------------------------
// Locked and unlocked.
//
// A command line process is one unlock and one run, so it never has these two
// states to be in. A server does: the Passphrase was spent at startup, and what
// it produced lives until a lock ends it (spec: DK-1). These are the cases over
// the moves between them — the lock somebody asks for, the one the clock makes,
// and what each of them does to work that is already running.
// ---------------------------------------------------------------------------

/// Every route that cannot answer without the Master Key, as a caller reaches
/// one.
///
/// The upload is not here and is asked separately: what it takes is a multipart
/// body, and a request without one is refused by the envelope rather than by the
/// lock — so the case that means to be about the lock sends a real drop.
const KEYED_ROUTES: [(&str, &str); 7] = [
    ("GET", "/api/folders"),
    ("GET", "/api/list?path=albums"),
    ("GET", "/api/file?path=albums/cover.png"),
    ("POST", "/api/fill?path=albums"),
    ("POST", "/api/sync"),
    ("POST", "/api/freeze?path=albums"),
    ("POST", "/api/refresh"),
];

/// The idle interval the cases about the clock run under.
///
/// A quarter of an hour, which is neither the default nor a constant of the
/// server: how long a device stays unlocked is a policy parameter (spec: DK-4),
/// and a case that used the shipped default would be testing the default rather
/// than the parameter.
const QUIET: Duration = Duration::from_secs(15 * 60);

/// One route asked, whichever verb it takes.
async fn route(served: &Served, method: &str, uri: &str) -> (axum::http::StatusCode, Value) {
    body_of(match method {
        "GET" => served.get(uri).await,
        _ => served.post(uri).await,
    })
    .await
}

// DK-3, and the whole of it in one case: the lock is available while the server
// is unlocked, and it has taken effect by the time it answers — the very next
// request finds nothing left to work with. The routes that never needed a key
// go on answering, which is what keeps a locked server something a person can
// still read the name of rather than a process that has gone silent.
#[tokio::test]
async fn an_explicit_lock_shuts_every_route_that_needs_a_key() {
    let served = Served::library().await;
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "it is open to begin with");

    let (status, locked) = body_of(served.post("/api/lock").await).await;
    assert_eq!(status, 200);
    assert_eq!(locked, json!({ "locked": true }));

    for (method, uri) in KEYED_ROUTES {
        let (status, refusal) = route(&served, method, uri).await;
        assert_eq!(status, 423, "{method} {uri} answered a locked server");
        assert_eq!(refusal["error"], "locked", "{method} {uri}");
    }

    let (status, library) = body_of(served.get("/api/library").await).await;
    assert_eq!(
        status, 200,
        "which Library this is is not a thing the Master Key keeps",
    );
    assert_eq!(library["name"], "served");
    let (status, _) = body_of(served.get("/api/activity").await).await;
    assert_eq!(
        status, 200,
        "and neither is this server's own account of what it was doing",
    );
}

// DK-2: none of them partially succeeds. A drop onto a locked server is refused
// whole — before a byte of any part reaches the folder — rather than landing
// files that no flow could ever carry in.
#[tokio::test]
async fn a_drop_onto_a_locked_server_lands_nothing() {
    let served = Served::library().await;
    let (status, _) = body_of(served.post("/api/lock").await).await;
    assert_eq!(status, 200);

    let (status, refusal) = body_of(
        served
            .upload("albums", &[("first.txt", b"one"), ("second.txt", b"two")])
            .await,
    )
    .await;
    assert_eq!(status, 423);
    assert_eq!(refusal["error"], "locked");
    assert!(!served.holds("albums/first.txt"), "and wrote nothing");
    assert!(!served.holds("albums/second.txt"));
}

// The sentence DK-2 asks for, said in the words the register uses, and said to
// somebody who can act on it: the Passphrase is what opens this, and starting
// the server again is where a Passphrase is typed. It is a kind of its own and
// not the admission fence's `unauthorized` — being locked is the owner's own
// state, not somebody else being turned away, so the answer tells them
// everything rather than nothing.
#[tokio::test]
async fn a_locked_server_says_the_passphrase_is_required() {
    let served = Served::library().await;
    let (status, _) = body_of(served.post("/api/lock").await).await;
    assert_eq!(status, 200);

    let (status, refusal) = body_of(served.get("/api/file?path=albums/cover.png").await).await;
    assert_eq!(status, 423);
    assert_eq!(refusal["error"], "locked");
    let said = refusal["message"]
        .as_str()
        .expect("a refusal carries one sentence")
        .to_owned();
    assert!(
        said.contains("Passphrase"),
        "it names what is needed: {said}"
    );
    assert!(
        said.contains("starting it again"),
        "and how to provide it: {said}",
    );
}

// Asking for a state rather than for an act. A second lock is not a failure and
// not a second wiping: it answers what the first one answered, and the server is
// in the same state it was already in.
#[tokio::test]
async fn locking_a_locked_server_answers_the_same() {
    let served = Served::library().await;

    for attempt in 1..=3 {
        let (status, locked) = body_of(served.post("/api/lock").await).await;
        assert_eq!(status, 200, "lock {attempt}");
        assert_eq!(locked, json!({ "locked": true }), "lock {attempt}");
    }

    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423);
    assert_eq!(refusal["error"], "locked");
}

// The other half of DK-3's "has taken effect by the time it returns", said about
// a server that answers many callers at once: what the lock ends is the next
// piece of work, not the one already running. A fetch that took its handle on
// the keys before the lock landed finishes with it and answers with the Entry —
// which is what "none of them partially succeeds" means per operation rather
// than per connection.
//
// The request is provably in flight rather than probably: Storage takes the read
// and holds it until this case lets go, so the lock lands while the fetch is
// inside the server and nowhere else.
#[tokio::test]
async fn a_request_in_flight_when_the_lock_lands_finishes() {
    let served = Served::library().await;
    served.hold_storage();

    let (answer, locked) =
        tokio::join!(served.get("/api/file?path=albums/2026/spring.jpg"), async {
            // No sleep and no guess: the read is counted as it arrives, so this
            // waits for the fetch to be inside Storage.
            while served.held_reads() == 0 {
                tokio::task::yield_now().await;
            }
            let locked = served.post("/api/lock").await;
            served.release_storage();
            locked
        },);

    assert_eq!(locked.status(), 200);
    assert_eq!(
        answer.status(),
        200,
        "the fetch that began unlocked finishes"
    );
    assert_eq!(bytes(answer).await, b"spring");
    assert!(
        served.holds("albums/2026/spring.jpg"),
        "and placed the file whole (spec: EP-11)",
    );

    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "what asks after the lock is refused");
    assert_eq!(refusal["error"], "locked");
}

// The work nobody asked for meets the same lock, and stops rather than half
// running: nothing is placed, and what the browser polls says why in the same
// words a refused request would have used. A fill that pressed on would be one
// Storage call per file to be told the same thing once per file.
#[tokio::test]
async fn background_work_that_meets_a_lock_stops_cleanly() {
    let served = Served::library().await;
    let (status, _) = body_of(served.post("/api/lock").await).await;
    assert_eq!(status, 200);

    served.arm_fill("albums");
    served.fill_settled().await;

    let (status, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(status, 200);
    let fill = &activity["fill"];
    assert_eq!(fill["status"], "stopped");
    assert_eq!(fill["done"], 0);
    assert_eq!(fill["stopped"]["error"], "locked");
    let said = fill["stopped"]["message"]
        .as_str()
        .expect("a stopped run carries one sentence");
    assert!(said.contains("Passphrase"), "{said}");
    assert!(
        !served.holds("albums/cover.png"),
        "and brought nothing over",
    );
}

// DK-4: inactivity for the configured interval locks the device. The clock is
// the case's own — `start_paused` is what lets a quarter of an hour be stated
// rather than spent — and nothing is asked of the server while it passes.
#[tokio::test(start_paused = true)]
async fn nothing_asked_for_the_idle_interval_locks_the_library() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;

    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "it is open while somebody is here");

    tokio::time::advance(QUIET + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;

    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 423,
        "and shut once nobody has been for the interval"
    );
    assert_eq!(refusal["error"], "locked");
    let (status, _) = body_of(served.get("/api/library").await).await;
    assert_eq!(status, 200, "the identity route answers either way");
}

// The other direction, and the one that makes the interval mean "quiet since
// somebody last wanted the Library" rather than "up for this long": three
// quarters of an hour pass in steps none of which is a quarter of an hour of
// silence, and the Library is still open at the end of them. What is asked is a
// route that needs the keys, because that is what being here means.
#[tokio::test(start_paused = true)]
async fn steady_activity_keeps_the_library_unlocked() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;

    for step in 1..=6 {
        tokio::time::advance(QUIET / 2).await;
        let (status, _) = route(&served, "GET", "/api/folders").await;
        assert_eq!(status, 200, "step {step}");
    }

    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 200,
        "no quarter of an hour of it was quiet, so nothing locked it",
    );
}

// And the mirror of it, which is the case the idle lock exists for: a tab left
// open on a page asks this server what it is doing several times a second, and
// none of that is a person at the keyboard. The same three quarters of an hour
// pass in the same steps — every one of them a request this server answers —
// and the Library locks anyway.
#[tokio::test(start_paused = true)]
async fn steady_polling_for_activity_does_not_keep_the_library_unlocked() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;

    for step in 1..=6 {
        tokio::time::advance(QUIET / 2).await;
        let answer = served.get("/api/activity").await;
        assert_eq!(
            answer.status(),
            200,
            "step {step}: the polling is answered either way",
        );
    }

    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 423,
        "nobody wanted the Library for a quarter of an hour, so it locked",
    );
    assert_eq!(refusal["error"], "locked");
}

// The interval is quiet since somebody last wanted the Library, and wanting it
// lasts as long as the work does. A book being packed can take longer than a
// quarter of an hour by itself, and a lock landing in the middle of that would
// leave the very work it interrupted with nowhere to go: the run finishes on the
// handle it holds, and everything it arms next is refused — so the explorer
// offers to pack again and cannot. Storage holds the read here for what a long
// piece of work is, and the interval is counted afresh from the end of it rather
// than from its first moment.
#[tokio::test(start_paused = true)]
async fn work_that_outlasts_the_interval_defers_the_lock() {
    let served = Served::library().await;
    served.watch_idle(QUIET).await;
    served.hold_storage();

    let (answer, ()) = tokio::join!(served.get("/api/file?path=albums/2026/spring.jpg"), async {
        // No sleep and no guess: the read is counted as it arrives, so the
        // clock is only moved once the handle on the keys is out.
        while served.held_reads() == 0 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(QUIET * 2).await;
        served.release_storage();
    },);
    assert_eq!(answer.status(), 200, "the long piece of work finishes");
    // A fetch arms a fill of the folder around it, and that run holds a handle
    // of its own: it is work over the Library in exactly the sense the request
    // was, so the quiet begins once it is done too.
    served.fill_settled().await;

    tokio::time::advance(QUIET / 2).await;
    tokio::task::yield_now().await;
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(
        status, 200,
        "and what comes after it is served, the interval starting at its end",
    );

    tokio::time::advance(QUIET + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "deferred, and not called off");
    assert_eq!(refusal["error"], "locked");
}

// Where the first interval begins. Opening the Library and catching its catalog
// up with what the Library has become both happen before the socket answers
// anything, and a Storage that answers slowly can make the second of them a
// wait of its own — none of it time anybody could have been at the keyboard for.
// The watcher marks the start as it begins to watch, so what the first interval
// measures is the first quiet a person could have kept: a server whose startup
// ran longer than the interval serves rather than arriving already locked.
#[tokio::test(start_paused = true)]
async fn the_first_interval_is_counted_from_where_the_serving_starts() {
    let served = Served::library().await;
    // What starting up cost, stated rather than spent, and all of it before
    // there is anything watching.
    tokio::time::advance(QUIET * 2).await;
    served.watch_idle(QUIET).await;

    tokio::time::advance(QUIET / 2).await;
    tokio::task::yield_now().await;
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "none of the startup was the interval");

    tokio::time::advance(QUIET + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    let (status, refusal) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 423, "and the quiet after somebody was here is");
    assert_eq!(refusal["error"], "locked");
}

// An interval nobody could reach the end of. The parser refuses nothing above
// its minimum, and the minutes are turned into seconds saturatingly, so a number
// large enough is a wait no clock can add up — which must leave the watcher
// waiting rather than take it out with a panic on the sum. A watcher that had
// panicked is a finished task, and a Library nothing will ever lock.
#[tokio::test(start_paused = true)]
async fn an_interval_longer_than_the_clock_neither_panics_nor_locks() {
    let served = Served::library().await;
    let watcher = served.watch_idle(Duration::from_secs(u64::MAX)).await;

    tokio::time::advance(Duration::from_secs(365 * 24 * 60 * 60)).await;
    tokio::task::yield_now().await;

    assert!(
        !watcher.is_finished(),
        "the watcher is still waiting rather than gone",
    );
    let (status, _) = route(&served, "GET", "/api/folders").await;
    assert_eq!(status, 200, "and a year of quiet was not the interval");
}
