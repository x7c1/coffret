//! An admitted request under `/api` through the host, against a stub server
//! that answers with what it was sent: the key, the authority and the origin
//! the real server admits a caller by (spec: LA-3, spec: LA-5), and bodies too
//! large to be gathered.

mod support;

use axum::body::{to_bytes, Body};
use axum::http::{Method, StatusCode};
use futures_util::stream;
use serde_json::{json, Value};
use support::{json_of, Harness, SERVER_KEY};

async fn sent(host: &Harness, request: axum::http::Request<Body>) -> Value {
    let response = host.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    json_of(response).await
}

#[tokio::test]
async fn the_key_is_read_again_for_every_request() {
    let host = Harness::start().await;
    let token = host.token().await;
    let reads = host.key.reads();

    for _ in 0..3 {
        let sent = sent(
            &host,
            host.with_token(Method::GET, "/api/headers", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(sent["headers"]["x-coffret-key"], json!([SERVER_KEY]));
    }
    assert_eq!(host.key.reads(), reads + 3);
}

#[tokio::test]
async fn an_absent_origin_stays_absent() {
    let host = Harness::start().await;
    let token = host.token().await;

    let sent = sent(
        &host,
        host.request(Method::GET, "/api/headers")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert!(sent["headers"]["origin"].is_null(), "{sent}");
    assert!(sent["headers"]["sec-fetch-site"].is_null(), "{sent}");
}

#[tokio::test]
async fn hop_by_hop_headers_are_not_forwarded() {
    let host = Harness::start().await;
    let token = host.token().await;

    let sent = sent(
        &host,
        host.with_token(Method::GET, "/api/headers", &token)
            .header("connection", "x-only-this-hop")
            .header("x-only-this-hop", "1")
            .header("proxy-authorization", "Basic c2VjcmV0")
            .header("x-end-to-end", "2")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert!(sent["headers"]["x-only-this-hop"].is_null());
    assert!(sent["headers"]["proxy-authorization"].is_null());
    assert_eq!(sent["headers"]["x-end-to-end"], json!(["2"]));
}

#[tokio::test]
async fn a_body_of_many_megabytes_goes_there_and_back_intact() {
    let host = Harness::start().await;
    let token = host.token().await;

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

    let response = host
        .send(
            host.with_token(Method::PUT, "/api/echo", &token)
                .body(body)
                .unwrap(),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let echoed = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the answer arrives in full");
    assert_eq!(echoed.len(), expected.len());
    assert!(echoed == expected, "the body came back altered");
}
