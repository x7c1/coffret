//! The listing routes: what one folder holds, every folder of the Library,
//! and which Library this is.

use serde_json::json;

use crate::support::{files, folders, json as body_of, states, Served};

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

// A folder of the Library is what the separators under it imply, so it cannot
// be empty: an empty listing means the path names nothing, which is what a
// mistyped component and a link kept too long both arrive as. The rows are
// identical to an empty Library root's, so the route says which of the two it
// answered — a browser reading the rows alone would show a mistyped path as an
// empty folder.
#[tokio::test]
async fn a_folder_the_library_does_not_have_is_told_from_an_empty_one() {
    let served = Served::library().await;

    let (status, mistyped) = body_of(served.get("/api/list?path=album").await).await;
    assert_eq!(status, 200);
    assert_eq!(mistyped["held"], false);
    assert_eq!(folders(&mistyped), Vec::<String>::new());
    assert_eq!(files(&mistyped), []);

    // A folder that does hold something, which is every folder there is.
    let (_, albums) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(albums["held"], true);

    // And the one empty folder a Library has: its root, before anything has been
    // committed into it. The lists are the mistyped path's exactly.
    let empty = Served::joined().await;
    let (status, root) = body_of(empty.get("/api/list").await).await;
    assert_eq!(status, 200);
    assert_eq!(root["held"], true, "the root is the Library and not a path");
    assert_eq!(folders(&root), Vec::<String>::new());
    assert_eq!(files(&root), []);
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
            ("theirs.jpg".to_owned(), "added".to_owned()),
        ],
    );
}
