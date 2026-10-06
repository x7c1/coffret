//! Every path outside `/api`: the explorer built into the binary under
//! `embed-web`, and a page saying it is missing without it.

use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use coffret_explorer_host::{router, Config};
use tower::ServiceExt;

async fn page(path: &str) -> (StatusCode, String, String) {
    let host = router(Config {
        server: "127.0.0.1:9".parse().unwrap(),
        key_file: "server-key-that-is-never-read".into(),
    })
    .expect("the host starts");
    let response = host
        .oneshot(
            Request::builder()
                .uri(path)
                .header("host", "127.0.0.1:5173")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("a router never fails");
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_owned())
        .unwrap_or_default();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the page is in memory");
    (
        status,
        content_type,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

#[cfg(feature = "embed-web")]
#[tokio::test]
async fn the_root_is_the_built_index_and_an_unknown_path_falls_back_to_it() {
    let (status, content_type, root) = page("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"), "{content_type}");
    assert!(
        root.contains("<html") || root.contains("<!doctype html"),
        "{root}"
    );

    let (status, _, deep) = page("/folders/somewhere/deep").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deep, root);
}

#[cfg(not(feature = "embed-web"))]
#[tokio::test]
async fn without_the_explorer_built_in_the_page_says_so() {
    let (status, _, page) = page("/").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(page.contains("not built into this binary"), "{page}");
}
