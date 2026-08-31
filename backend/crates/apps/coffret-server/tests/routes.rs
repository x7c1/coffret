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

    let (_, listing) = body_of(served.get("/api/list?path=albums/2026").await).await;
    assert_eq!(
        files(&listing)
            .into_iter()
            .map(|(name, state, ..)| (name, state))
            .collect::<Vec<_>>(),
        [
            ("spring.jpg".to_owned(), "present".to_owned()),
            ("summer.jpg".to_owned(), "remote".to_owned()),
        ],
    );
}

// A reader that opens a page and prefetches it, or two tabs on one folder: both
// requests answer with the Entry, and the Container is read once (spec: PK-16).
//
// What one fetch costs is measured rather than assumed — a range read of one
// Entry is several reads of one object, and how many is the fetch's business
// and not this case's. So the case asks for one Entry on its own, then for a
// second Entry twice at once, and the two have to cost the same: the Containers
// are the same shape, so a second caller that ran the flow again would show up
// as double.
#[tokio::test]
async fn one_entry_asked_for_twice_at_once_is_fetched_once() {
    let served = Served::library().await;

    let alone = served.get("/api/file?path=albums/2026/spring.jpg").await;
    assert_eq!(alone.status(), 200);
    let once = served.ranged_reads();
    assert!(once > 0, "fetching an Entry reads part of its Container");

    let (first, second) = served
        .get_twice("/api/file?path=albums/2026/summer.jpg")
        .await;
    assert_eq!(first.status(), 200);
    assert_eq!(second.status(), 200);
    assert_eq!(bytes(first).await, b"summer");
    assert_eq!(bytes(second).await, b"summer");
    assert_eq!(
        served.ranged_reads() - once,
        once,
        "the second caller waits for the first's verdict rather than reading again",
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
