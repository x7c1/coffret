//! The parts of an answer the flows read, taken out of the JSON the routes
//! send.

use super::{json as body_of, Served};

/// The rows of one listing, as `(name, state, container, openable)`.
///
/// A row with no Container of its own is `""`: nothing has been committed for a
/// file somebody just added, so what it will live in is the next sync's answer.
pub fn files(listing: &serde_json::Value) -> Vec<(String, String, String, bool)> {
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
pub fn states(listing: &serde_json::Value) -> Vec<(String, String)> {
    files(listing)
        .into_iter()
        .map(|(name, state, ..)| (name, state))
        .collect()
}

/// What one folder holds, as the listing route answers it.
pub async fn listing_of(served: &Served, folder: &str) -> serde_json::Value {
    let (status, listing) = body_of(served.get(&format!("/api/list?path={folder}")).await).await;
    assert_eq!(status, 200, "the listing of {folder} answers");
    listing
}

/// The names of a listing's child folders.
pub fn folders(listing: &serde_json::Value) -> Vec<String> {
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
pub fn folders_mapped(listing: &serde_json::Value) -> Vec<(String, bool)> {
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

/// The fill the server is on, or `null`.
pub fn fill(work: &serde_json::Value) -> &serde_json::Value {
    &work["fill"]
}

/// A work answer with the name of the process that answered taken out of it.
///
/// What names this process is drawn afresh every time one starts, so a case
/// comparing a whole answer cannot state it — and one that left the field in
/// would compare the one value that is different on every run. That the field
/// is there and is a name is stated on its own, by
/// [`crate::work::every_answer_says_which_process_it_came_from`].
pub fn without_server(work: &serde_json::Value) -> serde_json::Value {
    let mut without = work.clone();
    without
        .as_object_mut()
        .expect("an work is an object")
        .remove("server")
        .expect("an work says which process answered it");
    without
}

/// The Entries one fill declined, as `(path, reason)`.
pub fn declined(fill: &serde_json::Value) -> Vec<(String, String)> {
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

/// What the work answer says about the sync, which every drop arms.
pub fn sync(work: &serde_json::Value) -> &serde_json::Value {
    let sync = &work["sync"];
    assert!(!sync.is_null(), "a sync has been armed: {work}");
    sync
}

/// The names a drop wrote, as Entry Paths.
pub fn written(answer: &serde_json::Value) -> Vec<String> {
    answer["written"]
        .as_array()
        .expect("a drop says what it wrote")
        .iter()
        .map(|path| path.as_str().expect("an Entry Path").to_owned())
        .collect()
}

/// What the work answer says about the freeze, which a book drop arms.
pub fn freeze(work: &serde_json::Value) -> &serde_json::Value {
    let freeze = &work["freeze"];
    assert!(!freeze.is_null(), "a freeze has been armed: {work}");
    freeze
}

/// Every row of `folder`, as `(name, state, container)`.
pub async fn rows_of(served: &Served, folder: &str) -> Vec<(String, String, String)> {
    files(&listing_of(served, folder).await)
        .into_iter()
        .map(|(name, state, container, _)| (name, state, container))
        .collect()
}
