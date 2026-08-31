//! What the browser is answered with, over a Library another device filled.
//!
//! The router is driven as the service it is rather than through a socket, so
//! every case here is the whole path a request takes — the query, the device
//! layer, the catalog, and for one route the fetch that places a file — with
//! nothing standing in for any of it but the provider.

use serde_json::json;

mod support;
use support::{bytes, header, json as body_of, Served};

/// The rows of one listing, as `(name, state, container, openable)`.
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
                file["container"]
                    .as_str()
                    .expect("a row says which kind of Container holds it")
                    .to_owned(),
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

// An explorer that has opened nothing has nothing to be told about, and the
// route says so rather than inventing a fill nobody started.
#[tokio::test]
async fn nothing_is_being_filled_before_anything_is_opened() {
    let served = Served::library().await;

    let (status, activity) = body_of(served.get("/api/activity").await).await;
    assert_eq!(status, 200);
    assert_eq!(activity, json!({ "fill": null }));
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
    assert_eq!(activity, json!({ "fill": null }));
}
