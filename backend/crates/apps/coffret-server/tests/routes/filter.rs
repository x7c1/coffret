//! The filter: which rows a tree filtered by a name, by the most recently
//! deposited books, or by both keeps, out of the catalog alone.

use serde_json::{json, Value};

use crate::support::{json as body_of, route, Served};

/// A filter's query parameter as a URI carries it: every byte but an ASCII
/// letter or digit escaped, so a Japanese name or a space travels as itself.
fn escaped(text: &str) -> String {
    text.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// The rows of one answer, as `(path, kind)`.
fn rows(answer: &Value) -> Vec<(String, String)> {
    answer["rows"]
        .as_array()
        .expect("an answer carries its rows")
        .iter()
        .map(|row| {
            (
                row["path"].as_str().expect("a row has a path").to_owned(),
                row["kind"].as_str().expect("a row has a kind").to_owned(),
            )
        })
        .collect()
}

fn row(path: &str, kind: &str) -> (String, String) {
    (path.to_owned(), kind.to_owned())
}

async fn filtered(served: &Served, query: &str) -> Value {
    let (status, answer) = body_of(served.get(&format!("/api/filter?{query}")).await).await;
    assert_eq!(status, 200, "{query}: {answer}");
    answer
}

// LA-40: a name keeps the shallowest folder or file along each path whose
// segment it stands inside — not what a matched folder holds, nor the folders
// above it — case-insensitively and with full-width and half-width forms one.
// A folder carries the time the most recent books go by, `null` where it
// holds no file directly; a file carries none.
#[tokio::test]
async fn a_name_keeps_the_folders_and_files_it_matches() {
    let served = Served::library().await;
    served
        .commit_all_elsewhere(&[
            ("ノベル/スターフォール/表紙.jpg", b"a cover"),
            ("漫画/ｽﾀｰﾌｫｰﾙ/001.jpg", b"a page"),
        ])
        .await;
    served
        .commit_all_elsewhere(&[
            ("comics/Starfall/vol 01/001.jpg", b"one"),
            ("comics/Starfall/vol 02/001.jpg", b"two"),
            ("posters/starfall-map.png", b"a map"),
        ])
        .await;
    served.start_up().await;

    for typed in ["starfall", "STARFALL", "ｓｔａｒｆａｌｌ"] {
        let answer = filtered(&served, &format!("name={}", escaped(typed))).await;
        assert_eq!(
            rows(&answer),
            [
                row("comics/Starfall", "folder"),
                row("posters/starfall-map.png", "file"),
            ],
            "{typed}: {answer}",
        );
        assert_eq!(answer["rows"][0]["deposited_at"], Value::Null, "{answer}");
        assert!(
            answer["rows"][1].get("deposited_at").is_none(),
            "a file carries no book's time: {answer}",
        );
        assert_eq!(
            (answer["total"].clone(), answer["truncated"].clone()),
            (json!(2), json!(false))
        );
    }

    // The full-width katakana typed matches the half-width name, and the
    // other way round.
    for typed in ["スターフォール", "ｽﾀｰﾌｫｰﾙ"] {
        let answer = filtered(&served, &format!("name={}", escaped(typed))).await;
        assert_eq!(
            rows(&answer),
            [
                row("ノベル/スターフォール", "folder"),
                row("漫画/ｽﾀｰﾌｫｰﾙ", "folder"),
            ],
            "{typed}: {answer}",
        );
        // Both were deposited by the first commit after the fixture's own.
        for folder in answer["rows"].as_array().expect("rows") {
            assert_eq!(folder["deposited_at"], "2023-11-14T22:30:01Z", "{answer}");
        }
    }

    // Never across a `/`.
    let answer = filtered(&served, &format!("name={}", escaped("starfall/vol"))).await;
    assert_eq!(answer["total"], 0, "{answer}");
}

// LA-40: a name that matches nothing is an empty answer, not a refusal.
#[tokio::test]
async fn a_name_that_matches_nothing_keeps_nothing() {
    let served = Served::library().await;
    let answer = filtered(&served, "name=harbor").await;
    assert_eq!(
        answer,
        json!({ "rows": [], "total": 0, "truncated": false }),
    );
}

// LA-41: the n most recent books are the folders directly holding the files
// deposited last, in path order. The files directly under the root count as
// one book, the root's own, spelled as the empty path; a series holding only
// folders is no book.
#[tokio::test]
async fn the_most_recent_books_are_the_folders_deposited_last() {
    let served = Served::library().await;
    served
        .commit_elsewhere("series/vol 01/001.jpg", b"one")
        .await;
    served
        .commit_elsewhere("series/vol 02/001.jpg", b"two")
        .await;
    served.commit_elsewhere("loose-1.jpg", b"loose").await;
    served.commit_elsewhere("loose-2.jpg", b"looser").await;
    served.start_up().await;

    let answer = filtered(&served, "recent=2").await;
    assert_eq!(
        rows(&answer),
        [row("", "folder"), row("series/vol 02", "folder")],
        "{answer}",
    );
    assert_eq!(
        answer["rows"][0]["deposited_at"], "2023-11-14T22:30:04Z",
        "the root's newest file dates it: {answer}",
    );
    assert_eq!(answer["total"], 2);

    // Every book there is: the fixture's three, the two volumes and the root,
    // and never the series that holds the volumes.
    let answer = filtered(&served, "recent=50").await;
    assert_eq!(
        rows(&answer),
        [
            row("", "folder"),
            row("albums", "folder"),
            row("albums/2026", "folder"),
            row("books", "folder"),
            row("series/vol 01", "folder"),
            row("series/vol 02", "folder"),
        ],
        "{answer}",
    );
    assert_eq!(
        (answer["total"].clone(), answer["truncated"].clone()),
        (json!(6), json!(false))
    );
}

// LA-42: a name and a count together are the newest books named by a match or
// under one. A book holding only a matched file is neither.
#[tokio::test]
async fn a_name_and_a_count_take_the_newest_books_within_the_match() {
    let served = Served::library().await;
    for path in [
        "series/Starfall/vol 01/001.jpg",
        "series/Starfall/vol 02/001.jpg",
        "series/Starfall/vol 03/001.jpg",
        "other/vol 09/starfall.jpg",
        "other/newest/001.jpg",
    ] {
        served.commit_elsewhere(path, path.as_bytes()).await;
    }
    served.start_up().await;

    let answer = filtered(&served, "name=starfall&recent=2").await;
    assert_eq!(
        rows(&answer),
        [
            row("series/Starfall/vol 02", "folder"),
            row("series/Starfall/vol 03", "folder"),
        ],
        "{answer}",
    );
    assert_eq!(answer["total"], 2);
}

// LA-42: past 2,000 rows the answer stops, in path order, and says how many
// matched over the whole scan.
#[tokio::test]
async fn more_matches_than_the_limit_are_cut_short_and_counted() {
    let served = Served::library().await;
    let paths: Vec<String> = (0..2_005)
        .map(|i| format!("many/scan-{i:04}.png"))
        .collect();
    let files: Vec<(&str, &[u8])> = paths
        .iter()
        .map(|path| (path.as_str(), path.as_bytes()))
        .collect();
    served.commit_all_elsewhere(&files).await;
    served.start_up().await;

    let answer = filtered(&served, "name=scan").await;
    let kept = rows(&answer);
    assert_eq!(kept.len(), 2_000);
    assert_eq!(kept[0], row("many/scan-0000.png", "file"));
    assert_eq!(kept[1_999], row("many/scan-1999.png", "file"));
    assert_eq!(
        (answer["total"].clone(), answer["truncated"].clone()),
        (json!(2_005), json!(true))
    );
}

// LA-42: a filter that names neither, an empty name, and a count that is not
// a positive whole number are refused as malformed.
#[tokio::test]
async fn a_filter_that_is_not_one_is_refused() {
    let served = Served::library().await;
    for query in [
        "",
        "name=",
        "recent=0",
        "recent=five",
        "recent=-1",
        "name=albums&recent=0",
    ] {
        let (status, refusal) = route(&served, "GET", &format!("/api/filter?{query}")).await;
        assert_eq!(status, 400, "{query:?}: {refusal}");
        assert_eq!(refusal["error"], "bad_request", "{query:?}");
    }
}

// LA-42: refused while the Library is locked, as every route that reads the
// catalog is.
#[tokio::test]
async fn a_locked_library_is_not_filtered() {
    let served = Served::library().await;
    served.lock();
    let (status, refusal) = route(&served, "GET", "/api/filter?name=albums").await;
    assert_eq!(status, 423, "{refusal}");
    assert_eq!(refusal["error"], "locked");
}
