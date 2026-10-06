//! `/api` through the host, against a stub server that answers with what it was
//! sent: the key, the authority and the origin the real server admits a caller
//! by (spec: LA-3, spec: LA-5), and bodies too large to be gathered.

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use coffret_explorer_host::{router, Config};
use coffret_server::SERVER_KEY_HEADER;
use futures_util::stream;
use serde_json::{json, Map, Value};
use tempfile::TempDir;
use tower::ServiceExt;

/// Where the host this case drives is, as a browser would name it.
const HOST: &str = "127.0.0.1:5173";

/// A server on a port of its own that answers `/api/headers` with the request
/// line and every header it was sent, `/api/echo` with the body it was sent,
/// and `/api/redirect` with a redirect.
async fn stub() -> SocketAddr {
    let app = Router::new()
        .route("/api/headers", any(what_was_sent))
        .route(
            "/api/echo",
            any(|body: Body| async move { Body::from_stream(body.into_data_stream()) }),
        )
        .route(
            "/api/redirect",
            get(|| async { Redirect::to("/elsewhere") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port is free");
    let address = listener
        .local_addr()
        .expect("a bound listener has an address");
    tokio::spawn(async move { axum::serve(listener, app).await });
    address
}

async fn what_was_sent(request: Request) -> Response {
    let mut headers = Map::new();
    for name in request.headers().keys() {
        let values: Vec<Value> = request
            .headers()
            .get_all(name)
            .iter()
            .map(|value| Value::from(value.to_str().expect("the cases send text")))
            .collect();
        headers.insert(name.to_string(), Value::Array(values));
    }
    Json(json!({
        "method": request.method().as_str(),
        "uri": request.uri().to_string(),
        "headers": headers,
    }))
    .into_response()
}

/// A key file holding `key`, in a directory of the case's own.
fn key_file(key: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("server-key");
    std::fs::write(&file, key).expect("the key file is written");
    (dir, file)
}

fn host(server: SocketAddr, key_file: PathBuf) -> Router {
    router(Config { server, key_file }).expect("the host starts")
}

fn request(uri: &str) -> axum::http::request::Builder {
    Request::builder().uri(uri).header("host", HOST)
}

/// What the stub was sent, as it said it.
async fn sent(host: Router, request: Request) -> Value {
    let response = host.oneshot(request).await.expect("a router never fails");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the stub answers in full");
    serde_json::from_slice(&body).expect("the stub answers JSON")
}

fn header<'a>(sent: &'a Value, name: &str) -> Option<&'a Vec<Value>> {
    sent["headers"][name].as_array()
}

#[tokio::test]
async fn the_key_in_the_file_replaces_the_callers_and_the_rest_arrives_unchanged() {
    let server = stub().await;
    let (_dir, file) = key_file("the-key-in-the-file\n");

    let sent = sent(
        host(server, file),
        request("/api/headers?folder=a%20b&x=1")
            .method("POST")
            .header(SERVER_KEY_HEADER, "a-key-the-caller-made-up")
            .header("sec-fetch-site", "same-origin")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(
        header(&sent, SERVER_KEY_HEADER),
        Some(&vec![Value::from("the-key-in-the-file")]),
    );
    assert_eq!(sent["method"], "POST");
    assert_eq!(sent["uri"], "/api/headers?folder=a%20b&x=1");
    assert_eq!(
        header(&sent, "sec-fetch-site"),
        Some(&vec![Value::from("same-origin")]),
    );
}

#[tokio::test]
async fn the_key_is_read_again_for_every_request() {
    let server = stub().await;
    let (_dir, file) = key_file("the-first-run");
    let host = host(server, file.clone());

    let first = sent(
        host.clone(),
        request("/api/headers").body(Body::empty()).unwrap(),
    )
    .await;
    std::fs::write(&file, "the-second-run").expect("the key file is replaced");
    let second = sent(host, request("/api/headers").body(Body::empty()).unwrap()).await;

    assert_eq!(
        header(&first, SERVER_KEY_HEADER),
        Some(&vec![Value::from("the-first-run")])
    );
    assert_eq!(
        header(&second, SERVER_KEY_HEADER),
        Some(&vec![Value::from("the-second-run")])
    );
}

#[tokio::test]
async fn no_key_is_sent_when_the_file_is_not_there() {
    let server = stub().await;
    let dir = tempfile::tempdir().expect("a temporary directory");

    let sent = sent(
        host(server, dir.path().join("server-key")),
        request("/api/headers")
            .header(SERVER_KEY_HEADER, "a-key-the-caller-made-up")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(header(&sent, SERVER_KEY_HEADER), None);
}

#[tokio::test]
async fn the_host_header_names_the_server() {
    let server = stub().await;
    let (_dir, file) = key_file("key");

    let sent = sent(
        host(server, file),
        request("/api/headers").body(Body::empty()).unwrap(),
    )
    .await;

    assert_eq!(
        header(&sent, "host"),
        Some(&vec![Value::from(server.to_string())])
    );
}

#[tokio::test]
async fn the_hosts_own_origin_becomes_the_servers() {
    let server = stub().await;
    let (_dir, file) = key_file("key");

    let sent = sent(
        host(server, file),
        request("/api/headers")
            .header("origin", format!("http://{HOST}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(
        header(&sent, "origin"),
        Some(&vec![Value::from(format!("http://{server}"))]),
    );
}

#[tokio::test]
async fn an_origin_from_elsewhere_arrives_as_it_was() {
    let server = stub().await;
    let (_dir, file) = key_file("key");

    for elsewhere in [
        "https://example.com",
        "http://localhost:5173",
        "https://127.0.0.1:5173",
        "null",
    ] {
        let sent = sent(
            host(server, file.clone()),
            request("/api/headers")
                .header("origin", elsewhere)
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(
            header(&sent, "origin"),
            Some(&vec![Value::from(elsewhere)]),
            "{elsewhere} must be forwarded unchanged",
        );
    }
}

#[tokio::test]
async fn hop_by_hop_headers_are_not_forwarded() {
    let server = stub().await;
    let (_dir, file) = key_file("key");

    let sent = sent(
        host(server, file),
        request("/api/headers")
            .header("connection", "x-only-this-hop")
            .header("x-only-this-hop", "1")
            .header("proxy-authorization", "Basic c2VjcmV0")
            .header("x-end-to-end", "2")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(header(&sent, "x-only-this-hop"), None);
    assert_eq!(header(&sent, "proxy-authorization"), None);
    assert_eq!(header(&sent, "x-end-to-end"), Some(&vec![Value::from("2")]));
}

#[tokio::test]
async fn a_body_of_many_megabytes_goes_there_and_back_intact() {
    let server = stub().await;
    let (_dir, file) = key_file("key");

    // Twelve mebibytes in 64 KiB pieces, each piece unlike its neighbours so
    // that one dropped or repeated would show.
    let pieces: Vec<Vec<u8>> = (0..192_u32)
        .map(|index| {
            (0..65_536_u32)
                .map(|offset| (index.wrapping_mul(31) ^ offset) as u8)
                .collect()
        })
        .collect();
    let expected: Vec<u8> = pieces.concat();
    let body = Body::from_stream(stream::iter(
        pieces.into_iter().map(Ok::<_, std::convert::Infallible>),
    ));

    let response = host(server, file)
        .oneshot(request("/api/echo").method("PUT").body(body).unwrap())
        .await
        .expect("a router never fails");

    assert_eq!(response.status(), StatusCode::OK);
    let echoed = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the answer arrives in full");
    assert_eq!(echoed.len(), expected.len());
    assert!(echoed == expected, "the body came back altered");
}

#[tokio::test]
async fn a_redirect_is_handed_back_rather_than_followed() {
    let server = stub().await;
    let (_dir, file) = key_file("key");

    let response = host(server, file)
        .oneshot(request("/api/redirect").body(Body::empty()).unwrap())
        .await
        .expect("a router never fails");

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get("location").unwrap(), "/elsewhere");
}

#[tokio::test]
async fn a_server_that_is_not_answering_is_a_bad_gateway_naming_it() {
    // A port that was free a moment ago and that nothing listens on now.
    let server = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a loopback port is free");
    let (_dir, file) = key_file("key");

    let response = host(server, file)
        .oneshot(request("/api/list").body(Body::empty()).unwrap())
        .await
        .expect("a router never fails");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the refusal is short");
    let sentence = String::from_utf8(body.to_vec()).expect("the refusal is text");
    assert!(
        sentence.contains(&server.to_string()),
        "the refusal must name the address: {sentence}",
    );
    assert!(sentence.contains("not answering"), "{sentence}");
}
