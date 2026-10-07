//! Choosing a folder on this device to map: the folders inside one, its parent,
//! and the paths that name no folder to list (spec: EP-9, LA-2).

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::support::{json as body_of, Served};

/// The browse route's query for one path.
fn browse(path: &Path) -> String {
    let text = path.to_str().expect("a temporary path is text");
    format!("/api/browse?path={}", escaped(text))
}

/// `text` as a query value: every byte but the unreserved ones escaped.
fn escaped(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// A folder of the case's own, resolved as the route resolves what it lists.
fn tree() -> (tempfile::TempDir, std::path::PathBuf) {
    let made = tempfile::tempdir().expect("a temporary directory must be available");
    let root = made
        .path()
        .canonicalize()
        .expect("a temporary directory resolves");
    (made, root)
}

fn text(path: &Path) -> String {
    path.to_str().expect("a temporary path is text").to_owned()
}

// The listing: the folder as a whole path, the one above it, and the folders
// inside it by name — and nothing that is not a folder, nothing a symbolic link
// stands for, and nothing below the one level.
#[tokio::test]
async fn a_folder_lists_the_folders_directly_inside_it() {
    let served = Served::library().await;
    let (_made, root) = tree();
    for folder in ["scans", "albums", "albums/2026"] {
        fs::create_dir_all(root.join(folder)).expect("a temporary folder is writable");
    }
    fs::write(root.join("notes.txt"), b"a file").expect("a temporary file is writable");
    std::os::unix::fs::symlink(root.join("albums"), root.join("linked"))
        .expect("a symbolic link can be made");

    let (status, listed) = body_of(served.get(&browse(&root)).await).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(
        listed,
        json!({
            "path": text(&root),
            "parent": text(root.parent().expect("a temporary folder has a parent")),
            "folders": [
                { "name": "albums", "path": text(&root.join("albums")) },
                { "name": "scans", "path": text(&root.join("scans")) },
            ],
        }),
    );

    // A path that goes up and comes back down names the folder it ends at, as
    // the mapping of it would record it.
    let (status, again) = body_of(served.get(&browse(&root.join("albums/.."))).await).await;
    assert_eq!(status, 200, "{again}");
    assert_eq!(again["path"], text(&root));
}

// A folder whose name starts with a dot is left out, as a file manager leaves
// it out — the management area a mapped root carries among them (spec: EP-14).
#[tokio::test]
async fn a_dot_folder_is_left_out() {
    let served = Served::library().await;
    let (_made, root) = tree();
    for folder in [".cache", ".coffret", "visible"] {
        fs::create_dir(root.join(folder)).expect("a temporary folder is writable");
    }

    let (status, listed) = body_of(served.get(&browse(&root)).await).await;
    assert_eq!(status, 200, "{listed}");
    let names: Vec<&str> = listed["folders"]
        .as_array()
        .expect("a listing carries its folders")
        .iter()
        .map(|folder| folder["name"].as_str().expect("a folder has a name"))
        .collect();
    assert_eq!(names, ["visible"]);
}

// The root of the filesystem has no folder above it, and says so.
#[tokio::test]
async fn the_root_of_the_filesystem_has_no_parent() {
    let served = Served::library().await;

    let (status, listed) = body_of(served.get("/api/browse?path=/").await).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["path"], "/");
    assert_eq!(listed["parent"], Value::Null);
}

// A path that names no folder — nothing at all, a file, or text that is not a
// whole path — is the caller's to correct, and the sentence names it.
#[tokio::test]
async fn a_path_that_is_not_a_folder_is_refused_by_name() {
    let served = Served::library().await;
    let (_made, root) = tree();
    fs::write(root.join("notes.txt"), b"a file").expect("a temporary file is writable");

    for asked in [root.join("nowhere"), root.join("notes.txt")] {
        let (status, refusal) = body_of(served.get(&browse(&asked)).await).await;
        assert_eq!(status, 400, "{}: {refusal}", asked.display());
        assert_eq!(refusal["error"], "bad_request");
        let message = refusal["message"]
            .as_str()
            .expect("a refusal says something");
        assert!(
            message.contains(&text(&asked)),
            "the sentence names the path: {message}",
        );
    }

    let (status, refusal) = body_of(served.get("/api/browse?path=albums").await).await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");
}

// A folder the account this server runs as may not read is a `403`, apart from
// one that is not there: the person can see it in a file manager, and what
// stops them is its permissions.
#[tokio::test]
async fn a_folder_this_account_may_not_read_is_refused_as_such() {
    use std::os::unix::fs::PermissionsExt;

    let served = Served::library().await;
    let (_made, root) = tree();
    let shut = root.join("shut");
    fs::create_dir(&shut).expect("a temporary folder is writable");
    fs::set_permissions(&shut, fs::Permissions::from_mode(0o000))
        .expect("a temporary folder's mode can be changed");
    // An account the operating system lets read anything — a superuser — has no
    // folder it may not read, and this case has nothing to say about it.
    if fs::read_dir(&shut).is_ok() {
        fs::set_permissions(&shut, fs::Permissions::from_mode(0o755)).ok();
        return;
    }

    let (status, refusal) = body_of(served.get(&browse(&shut)).await).await;
    fs::set_permissions(&shut, fs::Permissions::from_mode(0o755))
        .expect("a temporary folder's mode can be put back");
    assert_eq!(status, 403, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");
    assert!(
        refusal["message"]
            .as_str()
            .is_some_and(|message| message.contains(&text(&shut))),
        "{refusal}",
    );
}
