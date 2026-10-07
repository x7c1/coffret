//! The contract with the explorer.
//!
//! Every answer a route gives that is not a refusal or the work answer, written out
//! as the wire carries it, for the explorer's own cases to read back through its
//! types. The third of the files that hold the two sides to one contract; the
//! other two are written by this crate's own cases, beside the refusals and the
//! work answer. These are driven through the routes against a real Library,
//! because a listing is the catalog, the mappings and the disk read together,
//! and building one by hand would be writing down what the listing is believed
//! to be rather than what it is.
//!
//! One thing is taken out before the file is compared, and only that: a file's
//! `mtime` is the time the fixture planted it, and differs every run, so it is
//! written as one fixed instant wherever it is not `null`.

use std::path::Path;

use serde_json::{json, Value};

use crate::support::{json as body_of, Served};

/// Where the explorer reads these answers from, relative to this crate.
const ANSWERS: &str = "../../../../frontend/packages/gateway/api/src/contract/answers.json";

/// What rewrites the committed file instead of comparing against it — the same
/// switch the crate's own contract cases take.
const WRITE: &str = "COFFRET_WRITE_CONTRACT";

/// The instant every planted file's time is written as.
const PLANTED_AT: &str = "2026-01-01T00:00:00Z";

/// `answer` with every file's time replaced by [`PLANTED_AT`].
fn steadied(mut answer: Value) -> Value {
    if let Some(files) = answer.get_mut("files").and_then(Value::as_array_mut) {
        for file in files {
            if file.get("mtime").is_some_and(|mtime| !mtime.is_null()) {
                file["mtime"] = Value::from(PLANTED_AT);
            }
        }
    }
    answer
}

/// What `uri` answers with, having answered `200`.
async fn answered_as_json(served: &Served, method: &str, uri: &str) -> Value {
    let response = match method {
        "GET" => served.get(uri).await,
        _ => served.post(uri).await,
    };
    let (status, body) = body_of(response).await;
    assert_eq!(status, 200, "{method} {uri}: {body}");
    steadied(body)
}

/// Where the case's own folder on this device is written in the file.
const PLACE: &str = "/home/someone/coffret";

/// `answer` with the case's folder written as [`PLACE`], and the folder above
/// it as the folder above that.
///
/// Every string is rewritten rather than named fields, because the folder
/// appears inside sentences as well as on its own.
fn placed(answer: Value, here: &str, parent: &str) -> Value {
    match answer {
        Value::String(text) if text == parent => Value::from("/home/someone"),
        Value::String(text) => Value::from(text.replace(here, PLACE)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| placed(item, here, parent))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(name, value)| (name, placed(value, here, parent)))
                .collect(),
        ),
        other => other,
    }
}

/// `value` with every object's fields in the order of their names.
///
/// Rendered that way whatever order the value was built in: a workspace build
/// can turn on `serde_json`'s `preserve_order` for every crate at once, and a
/// file whose field order followed the build would change with it.
fn canonical(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(fields) => {
            let mut names: Vec<&String> = fields.keys().collect();
            names.sort();
            serde_json::Value::Object(
                names
                    .into_iter()
                    .map(|name| (name.clone(), canonical(&fields[name])))
                    .collect(),
            )
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonical).collect())
        }
        other => other.clone(),
    }
}

/// Compares `written` with the committed file, or writes it where [`WRITE`] is
/// set.
fn held_to(relative: &str, written: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&canonical(written)).expect("a JSON value renders")
    );
    if std::env::var_os(WRITE).is_some() {
        std::fs::write(&path, rendered)
            .unwrap_or_else(|cause| panic!("{} must be writable: {cause}", path.display()));
        return;
    }
    let held = std::fs::read_to_string(&path)
        .unwrap_or_else(|cause| panic!("{} must be readable: {cause}", path.display()));
    assert!(
        held == rendered,
        "{} is not what this server sends. If the change means to alter the wire, run the \
         case again with {WRITE}=1 and read the file's diff as what the explorer will now \
         receive — then make its `contract.test.ts` pass over it. What this server sends \
         now:\n{rendered}",
        path.display(),
    );
}

// The explorer's half reads this file through its types; this is what holds the
// file to the server. A field that changes shape on this side fails here until
// the file follows.
#[tokio::test]
async fn the_answers_the_explorer_reads_are_the_ones_this_server_sends() {
    let served = Served::library().await;

    // A file this device has. Asking for it arms a fill of the folder that
    // holds it, and the case waits for that fill before any listing, so that
    // what the listings say does not depend on how far it got: every row of
    // `albums` the Library holds is `present`, and a row the Library holds and
    // this device does not is the packed listing's (`books`).
    served.get("/api/file?path=albums/notes.txt").await;
    served.fill_idle().await;
    // A file this device has and the Library does not yet: the third state,
    // which has no Container.
    served.plant_locally("albums/just-added.txt", b"local addition");
    // And an ordinary file standing where a folder would be, so that a drop
    // into that folder is refused per file.
    served.plant_locally("albums/blocked", b"a file in a folder's place");
    // And a folder the Library does not have, whose one file sits a level
    // further down: named in the listing's `folders_on_disk` and not among its
    // `folders`. The sync below commits the file, so the later `/api/folders`
    // and refresh answers count it as the Library's.
    served.plant_locally("albums/extras/page.jpg", b"a page in a subfolder");

    let root = answered_as_json(&served, "GET", "/api/list?path=").await;
    let albums = answered_as_json(&served, "GET", "/api/list?path=albums").await;
    let nowhere = answered_as_json(&served, "GET", "/api/list?path=nowhere").await;
    let upload = {
        let (status, body) = body_of(
            served
                .upload("albums", &[("dropped.jpg", b"a dropped file")])
                .await,
        )
        .await;
        assert_eq!(status, 200, "{body}");
        body
    };
    let refused = {
        let (status, body) = body_of(
            served
                .upload("albums/blocked", &[("page.jpg", b"a page")])
                .await,
        )
        .await;
        assert_eq!(status, 200, "{body}");
        body
    };
    served.sync_idle().await;

    // The root of a device that maps one top-level folder and not the root,
    // which is the listing of an unmapped root over mapped folders.
    let partial = Served::mapping_only("albums").await;
    let unmapped_root = answered_as_json(&partial, "GET", "/api/list?path=").await;
    // And a Library holding a Pack, whose rows name the other Container kind.
    let packed = Served::packed_library().await;
    let books = answered_as_json(&packed, "GET", "/api/list?path=books").await;

    // A folder on this device as the browse lists it, and the same device
    // mapping a top-level folder to one of those folders. Both carry local
    // paths, which differ every run, so the case's own folder is written as one
    // fixed place wherever it appears.
    let (browsed, mapped) = {
        let made = tempfile::tempdir().expect("a temporary directory must be available");
        let here = made
            .path()
            .canonicalize()
            .expect("a temporary directory resolves");
        for folder in ["albums", "scans"] {
            std::fs::create_dir(here.join(folder)).expect("a temporary folder is writable");
        }
        let here_text = here.to_str().expect("a temporary path is text").to_owned();
        let browsed =
            answered_as_json(&partial, "GET", &format!("/api/browse?path={here_text}")).await;
        let (status, mapped) = body_of(
            partial
                .post_json(
                    "/api/map",
                    &json!({ "local_root": format!("{here_text}/scans"), "prefix": "books" }),
                )
                .await,
        )
        .await;
        assert_eq!(status, 200, "{mapped}");
        let parent = here
            .parent()
            .and_then(Path::to_str)
            .expect("a temporary folder has a parent")
            .to_owned();
        (
            placed(browsed, &here_text, &parent),
            placed(mapped, &here_text, &parent),
        )
    };

    // The one answer that is `202`: a reconnect started, and the consent page it
    // hands the page to open.
    let (status, reconnecting) = body_of(served.post("/api/reconnect").await).await;
    assert_eq!(status, 202, "{reconnecting}");

    let written = json!({
        "library": answered_as_json(&served, "GET", "/api/library").await,
        "folders": answered_as_json(&served, "GET", "/api/folders").await,
        "listings": {
            "root": root,
            "albums": albums,
            "nowhere": nowhere,
            "unmapped_root": unmapped_root,
            "packed": books,
        },
        "uploads": {
            "written": upload,
            "refused": refused,
        },
        "refreshed": answered_as_json(&served, "POST", "/api/refresh").await,
        "reconnecting": reconnecting,
        "browsed": browsed,
        "mapped": mapped,
    });
    held_to(ANSWERS, &written);
}
