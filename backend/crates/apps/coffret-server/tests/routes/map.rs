//! Mapping a folder from the explorer: the Library root or a top-level folder
//! recorded through the device crate, seen by the next listing, and refused
//! where the folder cannot hold one (spec: EP-9, EP-13, LA-2).

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::support::{folders_mapped, json as body_of, Served};

fn text(path: &Path) -> String {
    path.to_str().expect("a temporary path is text").to_owned()
}

/// A folder of the case's own, resolved as a mapping records it.
fn folder() -> (tempfile::TempDir, std::path::PathBuf) {
    let made = tempfile::tempdir().expect("a temporary directory must be available");
    let root = made
        .path()
        .canonicalize()
        .expect("a temporary directory resolves");
    (made, root)
}

async fn map(served: &Served, local_root: &str, prefix: Value) -> (u16, Value) {
    let (status, body) = body_of(
        served
            .post_json(
                "/api/map",
                &json!({ "local_root": local_root, "prefix": prefix }),
            )
            .await,
    )
    .await;
    (status.as_u16(), body)
}

// EP-9: a device that maps one top-level folder has an unmapped Library root;
// mapping the root from the explorer is what the next listing reads, with no
// restart, and the folder now carries the identity the mapping expects of it
// (spec: EP-13).
#[tokio::test]
async fn mapping_the_library_root_is_seen_by_the_next_listing() {
    let served = Served::mapping_only("albums").await;
    let (_made, root) = folder();

    let (_, before) = body_of(served.get("/api/list").await).await;
    assert_eq!(before["mapped"], false);

    let (status, mapped) = map(&served, &text(&root), Value::Null).await;
    assert_eq!(status, 200, "{mapped}");
    assert_eq!(mapped["prefix"], Value::Null);
    assert_eq!(mapped["local_root"], text(&root));
    assert_eq!(mapped["replaced"], Value::Null);
    assert_eq!(mapped["marker"], "written");
    assert!(
        root.join(".coffret/root").is_file(),
        "the folder carries the marker a placement is checked against",
    );

    let (status, after) = body_of(served.get("/api/list").await).await;
    assert_eq!(status, 200, "{after}");
    assert_eq!(after["mapped"], true);
}

// EP-9: a top-level folder is mapped on its own, beside the one already mapped.
#[tokio::test]
async fn mapping_a_top_level_folder_is_seen_by_the_next_listing() {
    let served = Served::mapping_only("albums").await;
    let (_made, root) = folder();

    let (_, before) = body_of(served.get("/api/list?path=books").await).await;
    assert_eq!(before["mapped"], false);

    let (status, mapped) = map(&served, &text(&root), json!("books")).await;
    assert_eq!(status, 200, "{mapped}");
    assert_eq!(mapped["prefix"], "books");
    assert_eq!(mapped["replaced"], Value::Null);

    let (_, books) = body_of(served.get("/api/list?path=books").await).await;
    assert_eq!(books["mapped"], true);
    let (_, top) = body_of(served.get("/api/list").await).await;
    assert_eq!(
        folders_mapped(&top),
        [("albums".to_owned(), true), ("books".to_owned(), true)],
    );
}

// EP-9: mapping a prefix again moves it, and where it was is said back — the
// files under the old folder have just left the Library's reach on this
// device. A folder already carrying an identity keeps it (spec: EP-13).
#[tokio::test]
async fn a_mapping_it_replaced_is_said_back() {
    let served = Served::mapping_only("albums").await;
    let before = served
        .local_root()
        .canonicalize()
        .expect("the mapped folder resolves");
    let (_made, root) = folder();

    let (status, mapped) = map(&served, &text(&root), json!("albums")).await;
    assert_eq!(status, 200, "{mapped}");
    assert_eq!(mapped["replaced"], text(&before));
    assert_eq!(mapped["local_root"], text(&root));
    let message = mapped["message"]
        .as_str()
        .expect("an answer says something");
    assert!(
        message.contains(&text(&before)) && message.contains(&text(&root)),
        "the sentence names both folders: {message}",
    );

    // The folder it left carries a marker already, and mapping another part of
    // the Library to it adopts that identity rather than writing over it.
    let (status, adopted) = map(&served, &text(&before), json!("books")).await;
    assert_eq!(status, 200, "{adopted}");
    assert_eq!(adopted["marker"], "adopted");
}

// What the device crate refuses, the route refuses: a folder that is not there,
// a file, a path that is not whole, and a prefix naming more than one level —
// and none of them records anything.
#[tokio::test]
async fn a_folder_that_cannot_be_mapped_is_refused() {
    let served = Served::mapping_only("albums").await;
    let (_made, root) = folder();
    fs::write(root.join("notes.txt"), b"a file").expect("a temporary file is writable");

    for asked in [root.join("nowhere"), root.join("notes.txt")] {
        let (status, refusal) = map(&served, &text(&asked), json!("books")).await;
        assert_eq!(status, 400, "{}: {refusal}", asked.display());
        assert_eq!(refusal["error"], "bad_request");
        assert!(
            refusal["message"]
                .as_str()
                .is_some_and(|message| message.contains(&text(&asked))),
            "the sentence names the folder: {refusal}",
        );
    }

    let (status, refusal) = map(&served, "books", json!("books")).await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");

    let (status, refusal) = map(&served, &text(&root), json!("books/2026")).await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"], "bad_path");

    let (_, books) = body_of(served.get("/api/list?path=books").await).await;
    assert_eq!(books["mapped"], false, "nothing was recorded");
}

// EP-13: a folder whose management area is not coffret's cannot be given an
// identity, so no mapping is recorded over it — the request was read, and the
// folder is what it conflicts with.
#[tokio::test]
async fn a_folder_whose_management_area_is_not_coffrets_is_refused() {
    let served = Served::mapping_only("albums").await;
    let (_made, root) = folder();
    fs::write(root.join(".coffret"), b"not a folder").expect("a temporary file is writable");

    let (status, refusal) = map(&served, &text(&root), json!("books")).await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");

    let (_, books) = body_of(served.get("/api/list?path=books").await).await;
    assert_eq!(books["mapped"], false, "nothing was recorded");
}

// A body that is not the JSON the route takes is refused in the one shape a
// refusal takes, rather than in the extractor's own words.
#[tokio::test]
async fn a_body_that_is_not_a_mapping_is_refused() {
    let served = Served::mapping_only("albums").await;

    let (status, refusal) = body_of(served.post("/api/map").await).await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");

    let (status, refusal) = body_of(
        served
            .post_json("/api/map", &json!({ "prefix": "books" }))
            .await,
    )
    .await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");
}
