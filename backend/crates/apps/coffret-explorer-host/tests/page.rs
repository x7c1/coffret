//! Every path outside `/api` and `/auth`: the explorer built into the binary
//! under `embed-web`, and a page saying it is missing without it — each with
//! the headers that keep it from being framed, sniffed or followed by a
//! referrer (spec: LA-16).

mod support;

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, StatusCode};
use axum::response::Response;
use support::{text_of, Harness};

async fn page(path: &str) -> Response {
    let host = Harness::start().await;
    let response = host
        .send(host.request(Method::GET, path).body(Body::empty()).unwrap())
        .await;
    assert_eq!(host.key.reads(), 0);
    assert_eq!(host.upstream.connections(), 0);
    response
}

fn assert_page_headers(response: &Response) {
    let headers = response.headers();
    let policy = headers["content-security-policy"].to_str().unwrap();
    for directive in [
        "default-src 'self'",
        "connect-src 'self'",
        "img-src 'self' blob:",
        "object-src 'none'",
        "base-uri 'none'",
        "frame-ancestors 'none'",
        "form-action 'none'",
    ] {
        assert!(policy.contains(directive), "{directive} in {policy}");
    }
    assert!(!policy.contains("unsafe-eval"), "{policy}");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
}

#[cfg(feature = "embed-web")]
#[tokio::test]
async fn the_root_is_the_built_index_and_an_unknown_path_falls_back_to_it() {
    let response = page("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_page_headers(&response);
    let content_type = response.headers()[CONTENT_TYPE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(content_type.starts_with("text/html"), "{content_type}");
    let root = text_of(response).await;
    assert!(
        root.contains("<html") || root.contains("<!doctype html"),
        "{root}"
    );

    let deep = page("/folders/somewhere/deep").await;
    assert_eq!(deep.status(), StatusCode::OK);
    assert_eq!(text_of(deep).await, root);
}

#[cfg(not(feature = "embed-web"))]
#[tokio::test]
async fn without_the_explorer_built_in_the_page_says_so() {
    let response = page("/").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_page_headers(&response);
    assert!(response.headers()[CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    let page = text_of(response).await;
    assert!(page.contains("not built into this binary"), "{page}");
}

#[tokio::test]
async fn a_dotfile_or_a_traversal_is_not_served_whatever_was_built() {
    for path in ["/.env", "/%2e%2e/etc/passwd", "/assets/../.git/config"] {
        let response = page(path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert_page_headers(&response);
        assert_eq!(
            text_of(response).await,
            "There is nothing here.\n",
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_page_is_only_read() {
    let host = Harness::start().await;
    let response = host
        .send(host.request(Method::POST, "/").body(Body::empty()).unwrap())
        .await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
