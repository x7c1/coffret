//! What a refusal writes down.
//!
//! Every refusal these routes answer with records what the layer below reported,
//! and those reports are written for the person who owns the Library: a fetch's
//! refusals are identified by an Entry Path, and a drop this device could not
//! place names the folder standing in the way. Neither may reach a log file, for
//! the reasons the `Redacted` trait is written under.
//!
//! So each case here plants a name nothing else in the suite uses, drives the
//! request that used to carry it into the log, and reads the log back. Sentinels
//! rather than the fixture's own names, because `albums` and `cover.png` appear
//! in a dozen events legitimately and a search for one would prove nothing.

use axum::body::Body;
use coffret_logging::testing::CapturedLogs;
use tracing::Level;

use crate::support::{asking, json as body_of, refusal_of, Served};

/// An Entry Path no other case uses, planted to be looked for afterwards.
const SENTINEL_PATH: &str = "sentinel-folder-a41f/sentinel-entry-9c2e.jpg";

/// The folder on this device that stands in the way of one, likewise.
const SENTINEL_FOLDER: &str = "sentinel-local-7b30";

// EP-9: a path no mapping of this device reaches is refused by name to the
// browser and by kind to the log. The path is what identifies the refusal —
// which is exactly why the message names one and why the event must not.
#[tokio::test]
async fn an_unmapped_entry_path_is_recorded_by_its_shape_and_not_by_its_name() {
    let served = Served::mapping_only("albums").await;
    served
        .commit_elsewhere(SENTINEL_PATH, b"outside every mapping")
        .await;
    let logs = CapturedLogs::capture();

    let (status, refusal) =
        body_of(served.get(&format!("/api/file?path={SENTINEL_PATH}")).await).await;
    assert_eq!(status, 409);
    assert_eq!(refusal["reason"], "unmapped");

    // The refusal is in the file, and it is readable: which flow refused, and
    // which of the fetch's verdicts it was.
    let recorded = refusal_of(&logs, "answer");
    assert_eq!(
        recorded,
        format!("Fetch::UnmappedEntryPath(path_len={})", SENTINEL_PATH.len()),
    );
    logs.assert_free_of(&[SENTINEL_PATH, "sentinel-folder-a41f", "sentinel-entry-9c2e"]);
}

// EP-4, EP-11: a drop onto a folder an ordinary file stands where is refused
// per file, and the refusal names that folder to whoever is at the device. The
// event says a descent was blocked and neither path.
#[tokio::test]
async fn a_drop_a_blocked_folder_stopped_names_neither_path_in_the_log() {
    let served = Served::library().await;
    // Not a folder: a file, exactly where the folder would be.
    served.plant_locally(SENTINEL_FOLDER, b"an ordinary file, in a folder's place");
    let logs = CapturedLogs::capture();

    let (status, answer) = body_of(
        served
            .upload(SENTINEL_FOLDER, &[("sentinel-entry-9c2e.jpg", b"a page")])
            .await,
    )
    .await;
    assert_eq!(status, 200, "the drop was answered, per file: {answer}");
    assert_eq!(answer["refused"][0]["reason"], "unmaterializable");

    // Which of the two ways it could not be materialized is kept, because they
    // send a person to different places; the folder that stopped it is not.
    let recorded = refusal_of(&logs, "upload");
    assert_eq!(
        recorded,
        format!(
            "Fetch::UnmaterializablePath(path_len={}, descent=blocked)",
            SENTINEL_FOLDER.len() + "/sentinel-entry-9c2e.jpg".len()
        ),
    );
    logs.assert_free_of(&[SENTINEL_FOLDER, "sentinel-entry-9c2e"]);
}

// A drop whose connection went while one of its files was in flight. Nothing
// refused that file — it stopped arriving — so nothing is recorded as a part
// refused: the request ends the way a stream that broke between parts ends
// it, answered once and recorded once, as the answer. What had been written of
// the file went to a scratch name and is gone with it (spec: EP-11).
#[tokio::test]
async fn a_drop_whose_body_stopped_mid_file_records_no_file_as_refused() {
    let served = Served::library().await;
    let head = "--coffret-case-boundary\r\nContent-Disposition: form-data; name=\"file\"; \
                filename=\"broken.txt\"\r\nContent-Type: application/octet-stream\r\n\r\n\
                the first bytes of it";
    // The failure comes a poll after the part's headers, so the route has the
    // part in hand and is reading its bytes when the stream breaks — rather
    // than meeting the break while still looking for the part.
    let body = Body::from_stream(futures_util::stream::unfold(0u8, move |step| async move {
        match step {
            0 => Some((Ok(axum::body::Bytes::from_static(head.as_bytes())), 1)),
            1 => {
                tokio::task::yield_now().await;
                Some((
                    Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "the connection went",
                    )),
                    2,
                ))
            }
            _ => None,
        }
    }));
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(
        served
            .send(
                asking("POST", "/api/upload?path=albums")
                    .header(
                        "content-type",
                        "multipart/form-data; boundary=coffret-case-boundary",
                    )
                    .body(body)
                    .expect("a multipart request is well formed"),
            )
            .await,
    )
    .await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"], "bad_request");

    let per_part: Vec<_> = logs
        .at(Level::ERROR)
        .into_iter()
        .filter(|event| event.field("operation") == "upload")
        .collect();
    assert!(per_part.is_empty(), "no file was refused:\n{}", logs.text());
    let answered = refusal_of(&logs, "answer");
    assert!(answered.starts_with("Multipart"), "{answered}");
    assert!(
        !served
            .folder_names("albums")
            .iter()
            .any(|name| name.contains("broken")),
        "nothing of the file stayed: {:?}",
        served.folder_names("albums"),
    );
}

// EP-5: the Library holding nothing at a path is an answer about the request
// rather than a failure underneath it, so there is nothing to record — and the
// path the caller named is not written down on the way to saying so.
#[tokio::test]
async fn a_path_the_library_holds_nothing_at_leaves_no_trace_of_it() {
    let served = Served::library().await;
    let logs = CapturedLogs::capture();

    let (status, refusal) =
        body_of(served.get(&format!("/api/file?path={SENTINEL_PATH}")).await).await;
    assert_eq!(status, 404);
    assert_eq!(refusal["error"], "no_such_entry");

    logs.assert_free_of(&[SENTINEL_PATH, "sentinel-folder-a41f", "sentinel-entry-9c2e"]);
}

// The background half of the same rule. A fill answers nobody, so what it met
// reaches a person only through the run it publishes and the log it
// writes — and it writes through the very same recording a route uses. So the
// folder it was walking and the Entry it stopped on are as absent from its
// events as they are from a request's, while what stopped it is not.
#[tokio::test]
async fn a_fill_stopped_by_storage_names_neither_the_folder_nor_the_entry() {
    let served = Served::library().await;
    served
        .commit_elsewhere(SENTINEL_PATH, b"one Entry, and the folder holds no other")
        .await;
    // The served device has to know the Library moved before a fill can want
    // anything out of it.
    let (status, _) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);

    let logs = CapturedLogs::capture();
    served.halt_storage();
    served.arm_fill("sentinel-folder-a41f");
    served.fill_idle().await;

    // It really did stop on that Entry, so this case is asking about a refusal
    // that happened rather than about a run that found nothing to do.
    let (_, stopped) = body_of(served.get("/api/work").await).await;
    assert_eq!(stopped["fill"]["status"], "stopped", "{stopped}");
    assert_eq!(stopped["fill"]["stopped"]["error"], "storage");

    // What stopped it is in the file, in Storage's own words, and neither the
    // folder that was being walked nor the Entry it stopped on is.
    let recorded = refusal_of(&logs, "fill");
    assert!(recorded.starts_with("Fetch::"), "{recorded}");
    logs.assert_free_of(&[SENTINEL_PATH, "sentinel-folder-a41f", "sentinel-entry-9c2e"]);
}

// EP-9, EP-13: a mapping the explorer asked for and the device crate refused
// names the folder to the person who chose it, and the log says which refusal
// it was without the folder.
#[tokio::test]
async fn a_refused_mapping_names_the_folder_to_the_browser_and_not_to_the_log() {
    let served = Served::mapping_only("albums").await;
    let made = tempfile::tempdir().expect("a temporary directory must be available");
    let folder = made.path().join(SENTINEL_FOLDER);
    std::fs::create_dir(&folder).expect("a temporary folder is writable");
    // A management area that is not coffret's, so the identity cannot be given.
    std::fs::write(folder.join(".coffret"), b"not a folder").expect("a temporary file is writable");
    let logs = CapturedLogs::capture();

    let (status, refusal) = body_of(
        served
            .post_json(
                "/api/map",
                &serde_json::json!({
                    "local_root": folder.to_str().expect("a temporary path is text"),
                    "prefix": "books",
                }),
            )
            .await,
    )
    .await;
    assert_eq!(status, 409, "{refusal}");
    assert!(
        refusal["message"]
            .as_str()
            .is_some_and(|message| message.contains(SENTINEL_FOLDER)),
        "the person who chose the folder is told which: {refusal}",
    );

    let recorded = refusal_of(&logs, "answer");
    assert!(
        recorded.starts_with("Device::ManagementAreaNotADirectory"),
        "{recorded}"
    );
    logs.assert_free_of(&[SENTINEL_FOLDER]);
}
