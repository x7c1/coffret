//! What the host's events carry when a request cannot be forwarded, when an
//! answer breaks off, and when the key cannot be read: what failed and what
//! kind of failure it was, and never an Entry Path, any part of the request's
//! URL, or where the key file is (spec: EL-1).

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::body::{to_bytes, Body, Bytes};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::routing::any;
use axum::Router;
use coffret_explorer_host::{router, Config};
use coffret_logging::testing::{CapturedLogs, LoggedEvent};
use futures_util::{stream, StreamExt};
use tempfile::TempDir;
use tower::ServiceExt;
use tracing::Level;

/// The events this crate emits, and nobody else's: what the HTTP stack records
/// about itself is not what these cases are about.
const TARGET: &str = "coffret_explorer_host";

/// An Entry Path no event may hold, in the query the explorer puts one in.
const ENTRY_PATH: &str = "AUDIT_PRIVATE_BOOK";
const FILE_REQUEST: &str = "/api/file?path=AUDIT_PRIVATE_BOOK%2Fpage.jpg";

/// The pieces of [`FILE_REQUEST`] that no event may hold either.
const URL_PARTS: &[&str] = &[ENTRY_PATH, "page.jpg", "/api/file", "path=", "%2F"];

/// A device-local Library name no event may hold.
const LIBRARY_NAME: &str = "AUDIT_PRIVATE_LIBRARY";

fn host(server: SocketAddr, key_file: PathBuf) -> Router {
    router(Config { server, key_file }).expect("the host starts")
}

fn request(uri: &str) -> Request {
    Request::builder()
        .uri(uri)
        .header("host", "127.0.0.1:5173")
        .body(Body::empty())
        .unwrap()
}

/// A Library's directory named [`LIBRARY_NAME`], and the key file in it,
/// which nothing has written yet.
fn library() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let library = dir.path().join(LIBRARY_NAME);
    std::fs::create_dir(&library).expect("the Library's directory is made");
    (dir, library.join("server-key"))
}

/// A server that answers `/api/file` with part of a body and then breaks off,
/// and every other path with an empty `200`.
async fn stub() -> SocketAddr {
    let app = Router::new()
        .route(
            "/api/file",
            any(|| async {
                let first = stream::once(async {
                    Ok::<_, std::io::Error>(Bytes::from_static(b"the first half"))
                });
                // Held back long enough for the status and the first piece to
                // have gone, so that what breaks is the body and not the answer.
                let broken = stream::once(async {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    Err(std::io::Error::other("the server stopped midway"))
                });
                Body::from_stream(first.chain(broken))
            }),
        )
        .fallback(|| async { StatusCode::OK });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port is free");
    let address = listener
        .local_addr()
        .expect("a bound listener has an address");
    tokio::spawn(async move { axum::serve(listener, app).await });
    address
}

#[tokio::test]
async fn a_request_that_cannot_be_sent_leaves_no_part_of_its_url_in_the_event() {
    // A port that was free a moment ago and that nothing listens on now.
    let server = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a loopback port is free");
    let (_dir, key_file) = library();
    std::fs::write(&key_file, "key").expect("the key file is written");
    let logs = CapturedLogs::capture_target(TARGET);

    let response = host(server, key_file)
        .oneshot(request(FILE_REQUEST))
        .await
        .expect("a router never fails");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    // The page's sentence is built apart from the event, and holds no part of
    // the URL either.
    let sentence = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a 502's body is whole");
    let sentence = String::from_utf8_lossy(&sentence);
    for part in URL_PARTS {
        assert!(
            !sentence.contains(part),
            "{part:?} reached the page: {sentence}"
        );
    }
    logs.assert_free_of(URL_PARTS);
    let event = logs.only(Level::WARN);
    assert_eq!(
        event.message(),
        "a request could not be forwarded to the server"
    );
    assert_eq!(event.field("kind"), "connect", "{event}");
    assert_eq!(event.field("operation"), "send_request", "{event}");
    assert_eq!(event.field("server"), server.to_string(), "{event}");
}

#[tokio::test]
async fn an_answer_that_breaks_off_leaves_no_part_of_the_url_in_the_event() {
    let server = stub().await;
    let (_dir, key_file) = library();
    std::fs::write(&key_file, "key").expect("the key file is written");
    let logs = CapturedLogs::capture_target(TARGET);

    let response = host(server, key_file)
        .oneshot(request(FILE_REQUEST))
        .await
        .expect("a router never fails");
    assert_eq!(response.status(), StatusCode::OK);
    to_bytes(response.into_body(), usize::MAX)
        .await
        .expect_err("the answer breaks off");

    logs.assert_free_of(URL_PARTS);
    let event = logs.only(Level::WARN);
    assert_eq!(event.message(), "the server's answer broke off midway");
    // The client reports an answer's body that broke off as one it could not
    // decode.
    assert_eq!(event.field("kind"), "decode", "{event}");
    assert_eq!(event.field("operation"), "stream_answer", "{event}");
}

/// Forwards one request with the key read from `key_file`, and the one event
/// it left.
async fn forwarded_with(key_file: PathBuf) -> (CapturedLogs, LoggedEvent) {
    let server = stub().await;
    let logs = CapturedLogs::capture_target(TARGET);

    let response = host(server, key_file.clone())
        .oneshot(request("/api/list"))
        .await
        .expect("a router never fails");

    assert_eq!(response.status(), StatusCode::OK, "forwarded without a key");
    let path = key_file.display().to_string();
    logs.assert_free_of(&[LIBRARY_NAME, &path, "server-key"]);
    let event = logs.only(Level::WARN);
    assert_eq!(
        event.message(),
        "the server's key could not be read; /api is forwarded without one",
    );
    (logs, event)
}

#[tokio::test]
async fn a_missing_key_file_is_said_to_be_missing_and_not_where_it_is() {
    let (_dir, key_file) = library();

    let (_logs, event) = forwarded_with(key_file).await;

    assert_eq!(event.field("problem"), "missing", "{event}");
    assert_eq!(event.field("io_error_kind"), "NotFound", "{event}");
}

#[tokio::test]
async fn an_unreadable_key_file_is_said_to_be_unreadable_and_not_where_it_is() {
    let (_dir, key_file) = library();
    // A directory where the file should be cannot be read as one, and unlike a
    // file without permissions it stays unreadable to a case run as root.
    std::fs::create_dir(&key_file).expect("a directory stands in the file's place");

    let (_logs, event) = forwarded_with(key_file).await;

    assert_eq!(event.field("problem"), "unreadable", "{event}");
}

#[tokio::test]
async fn a_key_that_cannot_be_a_header_is_said_to_be_malformed_and_not_what_it_is() {
    let (_dir, key_file) = library();
    std::fs::write(&key_file, "AUDIT_KEY\u{1}SECRET").expect("the key file is written");

    let (logs, event) = forwarded_with(key_file).await;

    assert_eq!(event.field("problem"), "malformed", "{event}");
    logs.assert_free_of(&["AUDIT_KEY", "SECRET"]);
}

#[tokio::test]
async fn a_key_file_that_is_not_text_is_said_to_be_malformed() {
    let (_dir, key_file) = library();
    std::fs::write(&key_file, [0xff, 0xfe, 0xfd]).expect("the key file is written");

    let (_logs, event) = forwarded_with(key_file).await;

    assert_eq!(event.field("problem"), "malformed", "{event}");
}
