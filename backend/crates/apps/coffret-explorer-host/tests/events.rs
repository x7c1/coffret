//! What the host's events carry: what failed and what kind of failure it was
//! when a request cannot be forwarded, when an answer breaks off, and when the
//! key cannot be confirmed; and, for authentication, a kind, a fixed reason
//! and counts — never an Entry Path, any part of a request's URL, where the key
//! file is, a ticket, a token, either one's digest, or an `Authorization`
//! value (spec: EL-1).

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Method, StatusCode};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use coffret_explorer_host::KeyFile;
use coffret_logging::testing::{CapturedLogs, LoggedEvent};
use sha2::{Digest, Sha256};
use support::{Harness, KeyDouble, Upstream};
use tempfile::TempDir;
use tracing::Level;

/// The events this crate emits, and nobody else's: what the HTTP stack records
/// about itself is not what these cases are about.
const TARGET: &str = "coffret_explorer_host";

/// An Entry Path no event may hold, in the query the explorer puts one in.
const ENTRY_PATH: &str = "AUDIT_PRIVATE_BOOK";
const FILE_REQUEST: &str = "/api/broken?path=AUDIT_PRIVATE_BOOK%2Fpage.jpg";

/// The pieces of [`FILE_REQUEST`] that no event may hold either.
const URL_PARTS: &[&str] = &[ENTRY_PATH, "page.jpg", "/api/broken", "path=", "%2F"];

/// A device-local Library name no event may hold.
const LIBRARY_NAME: &str = "AUDIT_PRIVATE_LIBRARY";

/// A port that was free a moment ago and that nothing listens on now.
async fn nobody() -> Upstream {
    let mut upstream = Upstream::start().await;
    upstream.address = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a loopback port is free");
    upstream
}

#[tokio::test]
async fn a_request_that_cannot_be_sent_leaves_no_part_of_its_url_in_the_event() {
    let host = Harness::in_front_of(nobody().await).await;
    let token = host.token().await;
    let logs = CapturedLogs::capture_target(TARGET);

    let response = host
        .send(
            host.with_token(Method::GET, FILE_REQUEST, &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;

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
    assert_eq!(
        event.field("server"),
        host.upstream.address.to_string(),
        "{event}"
    );
}

#[tokio::test]
async fn an_answer_that_breaks_off_leaves_no_part_of_the_url_in_the_event() {
    let host = Harness::start().await;
    let token = host.token().await;
    let logs = CapturedLogs::capture_target(TARGET);

    let response = host
        .send(
            host.with_token(Method::GET, FILE_REQUEST, &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
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

/// A Library's directory named [`LIBRARY_NAME`], and the key file in it,
/// which nothing has written yet.
fn library() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let library = dir.path().join(LIBRARY_NAME);
    std::fs::create_dir(&library).expect("the Library's directory is made");
    (dir, library.join("server-key"))
}

/// Exchanges a ticket with the key read from `key_file`, and the one warning
/// that left.
async fn exchanged_with(key_file: PathBuf) -> (CapturedLogs, LoggedEvent) {
    let host = Harness::reading(
        Upstream::start().await,
        Arc::new(KeyFile(key_file.clone())),
        KeyDouble::new(),
    )
    .await;
    let logs = CapturedLogs::capture_target(TARGET);

    let response = host.send(host.launch(&host.ticket())).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY, "fails closed");
    let path = key_file.display().to_string();
    logs.assert_free_of(&[LIBRARY_NAME, &path, "server-key"]);
    let event = logs.only(Level::WARN);
    assert_eq!(
        event.message(),
        "the server's key could not be confirmed; every session was revoked",
    );
    (logs, event)
}

#[tokio::test]
async fn a_missing_key_file_is_said_to_be_missing_and_not_where_it_is() {
    let (_dir, key_file) = library();

    let (_logs, event) = exchanged_with(key_file).await;

    assert_eq!(event.field("problem"), "missing", "{event}");
    assert_eq!(event.field("io_error_kind"), "NotFound", "{event}");
}

#[tokio::test]
async fn an_unreadable_key_file_is_said_to_be_unreadable_and_not_where_it_is() {
    let (_dir, key_file) = library();
    // A directory where the file should be cannot be read as one, and unlike a
    // file without permissions it stays unreadable to a case run as root.
    std::fs::create_dir(&key_file).expect("a directory stands in the file's place");

    let (_logs, event) = exchanged_with(key_file).await;

    assert_eq!(event.field("problem"), "unreadable", "{event}");
}

#[tokio::test]
async fn a_key_that_cannot_be_a_header_is_said_to_be_malformed_and_not_what_it_is() {
    let (_dir, key_file) = library();
    std::fs::write(&key_file, "AUDIT_KEY\u{1}SECRET").expect("the key file is written");

    let (logs, event) = exchanged_with(key_file).await;

    assert_eq!(event.field("problem"), "malformed", "{event}");
    logs.assert_free_of(&["AUDIT_KEY", "SECRET"]);
}

#[tokio::test]
async fn a_key_file_that_is_not_text_is_said_to_be_malformed() {
    let (_dir, key_file) = library();
    std::fs::write(&key_file, [0xff, 0xfe, 0xfd]).expect("the key file is written");

    let (_logs, event) = exchanged_with(key_file).await;

    assert_eq!(event.field("problem"), "malformed", "{event}");
}

/// Every rendering of the digest the host keeps of `credential`.
fn digests_of(credential: &str) -> Vec<String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(credential)
        .expect("a credential is base64url");
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    vec![
        hex.clone(),
        hex.to_uppercase(),
        STANDARD.encode(digest),
        URL_SAFE_NO_PAD.encode(digest),
    ]
}

// Issue, exchange, refusal and revocation each leave an event by kind, and
// none carries a ticket, a token, a digest of either, an `Authorization` value,
// a query or a path.
#[tokio::test]
async fn authentication_events_carry_kinds_reasons_and_counts_alone() {
    let host = Harness::start().await;
    let logs = CapturedLogs::capture_target(TARGET);

    let ticket = host.ticket();
    let launched = host.send(host.launch(&ticket)).await;
    assert_eq!(launched.status(), StatusCode::OK);
    let token = support::json_of(launched).await["token"]
        .as_str()
        .unwrap()
        .to_owned();

    // The same ticket again, and a token that is not one.
    assert_eq!(
        host.send(host.launch(&ticket)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let wrong = "AUDIT_WRONG_TOKEN_0000000000000000000000000";
    for request in [
        host.with_token(Method::GET, "/api/list?folder=AUDIT_QUERY", wrong),
        host.request(Method::GET, "/api/AUDIT_PATH")
            .header("origin", "https://AUDIT_ORIGIN.invalid"),
        host.page_request(Method::GET, "/auth/AUDIT_AUTH_PATH"),
    ] {
        let response = host.send(request.body(Body::empty()).unwrap()).await;
        assert!(response.status().is_client_error());
    }
    let read = host
        .send(
            host.with_token(Method::GET, "/api/list?folder=AUDIT_QUERY", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(read.status(), StatusCode::OK);
    let revoked = host
        .send(
            host.with_token(Method::POST, "/auth/session/revoke", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(revoked.status(), StatusCode::NO_CONTENT);
    host.credentials.revoke_all();

    let mut sentinels: Vec<String> = vec![
        ticket.clone(),
        token.clone(),
        format!("Launch {ticket}"),
        format!("Bearer {token}"),
        wrong.to_owned(),
        "AUDIT_QUERY".into(),
        "AUDIT_PATH".into(),
        "AUDIT_ORIGIN".into(),
        "AUDIT_AUTH_PATH".into(),
        "/api/list".into(),
        "/auth/launch".into(),
        "folder=".into(),
        support::SERVER_KEY.into(),
    ];
    sentinels.extend(digests_of(&ticket));
    sentinels.extend(digests_of(&token));
    let sentinels: Vec<&str> = sentinels.iter().map(String::as_str).collect();
    logs.assert_free_of(&sentinels);

    // Every event these paths leave is one about authentication, and says
    // which kind it is.
    let kinds: Vec<String> = logs
        .events()
        .iter()
        .map(|event| event.field("kind"))
        .collect();
    for kind in ["issue", "exchange", "refusal", "revocation"] {
        assert!(kinds.iter().any(|seen| seen == kind), "{kind} in {kinds:?}");
    }
    let refusals: Vec<String> = logs
        .events()
        .iter()
        .filter(|event| event.field("kind") == "refusal")
        .map(|event| event.field("reason"))
        .collect();
    for reason in ["ticket", "token", "origin", "auth_path"] {
        assert!(
            refusals.iter().any(|seen| seen == reason),
            "{reason} in {refusals:?}"
        );
    }
}

// LA-18: an exchange past the session cap records the session it revoked as a
// revocation by kind, a fixed reason and a count, and carries no ticket, token
// or digest of either, the evicted session's included.
#[tokio::test]
async fn an_eviction_at_the_session_cap_carries_a_kind_a_reason_and_a_count_alone() {
    let host = Harness::start().await;
    let logs = CapturedLogs::capture_target(TARGET);

    let mut credentials = Vec::new();
    for _ in 0..9 {
        let ticket = host.ticket();
        let launched = host.send(host.launch(&ticket)).await;
        assert_eq!(launched.status(), StatusCode::OK);
        let token = support::json_of(launched).await["token"]
            .as_str()
            .unwrap()
            .to_owned();
        credentials.push(ticket);
        credentials.push(token);
    }

    let mut sentinels: Vec<String> = Vec::new();
    for credential in &credentials {
        sentinels.push(credential.clone());
        sentinels.extend(digests_of(credential));
    }
    let sentinels: Vec<&str> = sentinels.iter().map(String::as_str).collect();
    logs.assert_free_of(&sentinels);

    let revocations: Vec<LoggedEvent> = logs
        .events()
        .into_iter()
        .filter(|event| event.field("kind") == "revocation")
        .collect();
    assert_eq!(revocations.len(), 1, "{revocations:?}");
    let eviction = &revocations[0];
    assert_eq!(eviction.field("reason"), "session_cap", "{eviction}");
    assert_eq!(eviction.number("revoked_sessions"), 1, "{eviction}");
}
