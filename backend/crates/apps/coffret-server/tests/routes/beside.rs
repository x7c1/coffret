//! The files either side of one row in reading order: what a reader at the
//! end of one folder reads on into, and steps back onto (spec: LA-32).

use serde_json::{json, Value};

use crate::support::{json as body_of, Served};

/// What the route answers for one row, having answered `200`.
async fn beside(served: &Served, uri: &str) -> Value {
    let (status, answer) = body_of(served.get(uri).await).await;
    assert_eq!(status, 200, "{answer}");
    answer
}

// LA-32: the file after the last of one folder is the first of the next, at
// whatever depth — a folder's folders come before its files — and the answer
// is the catalog's, so folders nothing has listed are answered for as readily
// and Storage is asked nothing. Both ends of the Library answer `null`.
#[tokio::test]
async fn the_files_beside_a_row_run_across_folders_nothing_has_listed() {
    let served = Served::library().await;
    served.forget_reads();

    // `albums/2026` comes before the files of `albums`, and `books` after them.
    assert_eq!(
        beside(&served, "/api/beside?path=albums/notes.txt").await,
        json!({ "previous": "albums/cover.png", "next": "books/page-001.png" }),
    );
    assert_eq!(
        beside(&served, "/api/beside?path=albums/2026/summer.jpg").await,
        json!({ "previous": "albums/2026/spring.jpg", "next": "albums/caf\u{e9}.jpg" }),
    );
    assert_eq!(
        beside(&served, "/api/beside?path=albums/2026/spring.jpg").await,
        json!({ "previous": null, "next": "albums/2026/summer.jpg" }),
        "nothing comes before the first file of the Library",
    );
    assert_eq!(
        beside(&served, "/api/beside?path=books/page-001.png").await,
        json!({ "previous": "albums/notes.txt", "next": null }),
        "nor after the last",
    );
    // A folder's place is just before everything under it.
    assert_eq!(
        beside(&served, "/api/beside?path=books&folder=true").await,
        json!({ "previous": "albums/notes.txt", "next": "books/page-001.png" }),
    );
    assert_eq!(
        beside(&served, "/api/beside?path=albums&folder=true").await,
        json!({ "previous": null, "next": "albums/2026/spring.jpg" }),
    );

    assert!(
        served.asked().is_empty(),
        "the catalog answers, and Storage is asked nothing: {:?}",
        served.asked(),
    );
    served.fill_idle().await;
    let (_, work) = body_of(served.get("/api/work").await).await;
    assert_eq!(work["fill"], Value::Null, "nothing was set going: {work}");
}

// LA-32: a row is named as a file is anywhere else, and one that is not a path
// is refused before the catalog is asked.
#[tokio::test]
async fn a_row_that_is_not_a_path_is_refused() {
    let served = Served::library().await;

    for uri in [
        "/api/beside",
        "/api/beside?path=",
        "/api/beside?path=albums//x",
    ] {
        let (status, refusal) = body_of(served.get(uri).await).await;
        assert_eq!(status, 400, "{uri}: {refusal}");
        assert_eq!(refusal["error"], "bad_path", "{uri}: {refusal}");
    }
}

// LA-32: only the catalog's current Entries are in reading order. A file
// somebody put into the mapped folder that no run has carried in takes no place
// between the files either side of it; and asked about itself — a row the
// Library holds nothing at — it is answered for the place its path names, as
// is a folder the Library holds nothing under.
#[tokio::test]
async fn a_row_the_library_holds_nothing_at_is_answered_for_its_place() {
    let served = Served::library().await;
    served.plant_locally("albums/d-added.jpg", b"something of my own");

    assert_eq!(
        beside(&served, "/api/beside?path=albums/cover.png").await,
        json!({ "previous": "albums/caf\u{e9}.jpg", "next": "albums/notes.txt" }),
        "the file standing in the mapped folder is not counted",
    );
    assert_eq!(
        beside(&served, "/api/beside?path=albums/d-added.jpg").await,
        json!({ "previous": "albums/cover.png", "next": "albums/notes.txt" }),
    );
    assert_eq!(
        beside(&served, "/api/beside?path=albums/2027&folder=true").await,
        json!({ "previous": "albums/2026/summer.jpg", "next": "albums/caf\u{e9}.jpg" }),
    );
}
