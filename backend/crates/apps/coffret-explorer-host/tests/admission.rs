//! What a request under `/api` passes before the host forwards it, against the
//! real router: the fixed authority, the origin fences, the browser token, and
//! the lock — and that a refused request reads no key and reaches no server
//! (spec: LA-13, LA-14, LA-15, LA-16).

mod support;

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::body::{Body, Bytes};
use axum::http::{Method, StatusCode};
use axum::response::Response;
use futures_util::Stream;
use serde_json::Value;
use support::{json_of, Harness, SERVER_KEY};

/// The headers a case sends, by name.
type Headers = Vec<(&'static str, String)>;

/// The same, borrowed from what the case already holds.
type Borrowed<'a> = Vec<(&'static str, &'a str)>;

/// What a refusal answers, and that it named nothing it was shown.
async fn refused_as(response: Response, status: StatusCode, error: &str) {
    assert_eq!(response.status(), status);
    assert!(
        response
            .headers()
            .keys()
            .all(|name| !name.as_str().starts_with("access-control-")),
        "{:?}",
        response.headers()
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = json_of(response).await;
    assert_eq!(body["error"], error, "{body}");
}

/// That nothing reached the key or the server.
fn untouched(host: &Harness) {
    assert_eq!(host.key.reads(), 0, "the key file was read");
    assert_eq!(host.upstream.connections(), 0, "the server was reached");
}

// The regression cases: each reached the server as its owner before the host
// admitted by token. Now every one is refused, and none costs the server
// anything (spec: LA-14).
#[tokio::test]
async fn the_requests_that_reached_the_server_as_its_owner_are_refused() {
    let host = Harness::start().await;
    let evil = "evil.invalid:5173";
    let cases: [(&str, Headers, StatusCode, &str); 5] = [
        (
            "loopback Host, same Origin, same-origin",
            vec![
                ("host", host.authority.clone()),
                ("origin", host.origin.clone()),
                ("sec-fetch-site", "same-origin".into()),
            ],
            StatusCode::UNAUTHORIZED,
            "authentication_required",
        ),
        (
            "loopback Host, other Origin, cross-site",
            vec![
                ("host", host.authority.clone()),
                ("origin", "https://evil.invalid".into()),
                ("sec-fetch-site", "cross-site".into()),
            ],
            StatusCode::FORBIDDEN,
            "origin_refused",
        ),
        (
            "other Host, matching Origin, same-origin",
            vec![
                ("host", evil.into()),
                ("origin", format!("http://{evil}")),
                ("sec-fetch-site", "same-origin".into()),
            ],
            StatusCode::FORBIDDEN,
            "origin_refused",
        ),
        (
            "other Host, matching Origin, no Fetch Metadata",
            vec![("host", evil.into()), ("origin", format!("http://{evil}"))],
            StatusCode::FORBIDDEN,
            "origin_refused",
        ),
        (
            "loopback Host, no Origin and no Fetch Metadata",
            vec![("host", host.authority.clone())],
            StatusCode::UNAUTHORIZED,
            "authentication_required",
        ),
    ];

    for (case, headers, status, error) in cases {
        for method in [Method::GET, Method::POST] {
            let mut request = axum::http::Request::builder()
                .method(method.clone())
                .uri("/api/list?folder=a");
            for (name, value) in &headers {
                request = request.header(*name, value);
            }
            let response = host.send(request.body(Body::empty()).unwrap()).await;
            assert_eq!(response.status(), status, "{case}, {method}");
            refused_as(response, status, error).await;
        }
    }
    untouched(&host);
}

/// A body that says whether anything ever asked it for a byte.
struct Watched {
    read: Arc<AtomicBool>,
}

impl Stream for Watched {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.read.store(true, Ordering::SeqCst);
        Poll::Ready(Some(Ok(Bytes::from_static(&[0; 1024]))))
    }
}

// Reads, mutations, an upload and a path no route answers are all one refusal
// without a token, and an upload's body is never read (spec: LA-14, LA-19).
#[tokio::test]
async fn without_a_token_nothing_under_api_is_answered() {
    let host = Harness::start().await;
    for (method, path) in [
        (Method::GET, "/api/list"),
        (Method::POST, "/api/sync"),
        (Method::GET, "/api/no-such-route"),
        (Method::GET, "/api"),
        (Method::GET, "/api/"),
        (Method::DELETE, "/api/file"),
    ] {
        let response = host
            .send(
                host.page_request(method.clone(), path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        refused_as(
            response,
            StatusCode::UNAUTHORIZED,
            "authentication_required",
        )
        .await;
    }

    let read = Arc::new(AtomicBool::new(false));
    let upload = host
        .page_request(Method::POST, "/api/upload?folder=a")
        .header("content-type", "multipart/form-data; boundary=x")
        .header("content-length", (1_u64 << 30).to_string())
        .body(Body::from_stream(Watched {
            read: Arc::clone(&read),
        }))
        .unwrap();
    let response = host.send(upload).await;
    assert_eq!(response.headers()["connection"], "close");
    refused_as(
        response,
        StatusCode::UNAUTHORIZED,
        "authentication_required",
    )
    .await;
    assert!(!read.load(Ordering::SeqCst), "the upload's body was read");
    untouched(&host);
}

// LA-13: the authority is the bound address exactly, and nothing a caller says
// about a proxy in front of it changes that.
#[tokio::test]
async fn a_host_other_than_the_bound_authority_is_refused() {
    let host = Harness::start().await;
    let token = host.token().await;
    let reads = host.key.reads();
    let port = host.authority.rsplit_once(':').unwrap().1.to_owned();
    let other_port = format!("127.0.0.1:{}", port.parse::<u16>().unwrap() ^ 1);
    let localhost = format!("localhost:{port}");

    let named: Vec<(&str, Vec<String>, &str)> = vec![
        ("missing", vec![], "/api/list"),
        (
            "duplicated",
            vec![host.authority.clone(), host.authority.clone()],
            "/api/list",
        ),
        ("localhost", vec![localhost], "/api/list"),
        ("another port", vec![other_port], "/api/list"),
        ("another name", vec!["evil.invalid".into()], "/api/list"),
        (
            "an absolute target naming elsewhere",
            vec![host.authority.clone()],
            "http://evil.invalid/api/list",
        ),
    ];
    for (case, hosts, target) in named {
        let mut request = axum::http::Request::builder()
            .uri(target)
            .header("authorization", format!("Bearer {token}"));
        for value in hosts {
            request = request.header("host", value);
        }
        let response = host.send(request.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{case}");
    }

    for (name, value) in [
        ("forwarded", "host=127.0.0.1"),
        ("x-forwarded-host", "127.0.0.1"),
    ] {
        let response = host
            .send(
                axum::http::Request::builder()
                    .uri("/api/list")
                    .header("host", "evil.invalid")
                    .header(name, value.replace("127.0.0.1", &host.authority))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{name}");
    }

    // A page under another name is not served the explorer's page either.
    let page = host
        .send(
            axum::http::Request::builder()
                .uri("/")
                .header("host", "evil.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(page.status(), StatusCode::FORBIDDEN);
    assert_eq!(host.key.reads(), reads, "the key file was read");
    assert_eq!(host.upstream.connections(), 0, "the server was reached");
}

// LA-14, LA-16: what a browser says about where a request came from keeps
// another site's page out, a token or not, and no answer is a CORS one.
#[tokio::test]
async fn a_request_from_another_site_is_refused_whatever_it_carries() {
    let host = Harness::start().await;
    let token = host.token().await;
    let bearer = format!("Bearer {token}");

    let cases: Vec<(&str, Borrowed, Method)> = vec![
        ("null Origin", vec![("origin", "null")], Method::GET),
        (
            "two Origins",
            vec![("origin", &host.origin), ("origin", &host.origin)],
            Method::GET,
        ),
        (
            "same-site",
            vec![("sec-fetch-site", "same-site")],
            Method::GET,
        ),
        (
            "cross-site",
            vec![("sec-fetch-site", "cross-site")],
            Method::GET,
        ),
        (
            "a form POST from another site",
            vec![
                ("origin", "https://evil.invalid"),
                ("sec-fetch-site", "cross-site"),
                ("content-type", "application/x-www-form-urlencoded"),
            ],
            Method::POST,
        ),
        (
            "a preflight",
            vec![
                ("origin", "https://evil.invalid"),
                ("access-control-request-method", "POST"),
            ],
            Method::OPTIONS,
        ),
    ];
    for (case, headers, method) in cases {
        let mut request = host
            .request(method, "/api/sync")
            .header("authorization", &bearer);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = host.send(request.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{case}");
        refused_as(response, StatusCode::FORBIDDEN, "origin_refused").await;
    }

    // A preflight with no token, from the page's own origin, is not answered
    // as one either.
    let preflight = host
        .send(
            host.page_request(Method::OPTIONS, "/api/sync")
                .header("access-control-request-method", "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    refused_as(
        preflight,
        StatusCode::UNAUTHORIZED,
        "authentication_required",
    )
    .await;
    assert_eq!(host.upstream.connections(), 0);
}

// LA-14: the origin fences are not authentication. A client that is not a
// browser writes them as it likes, and without a token is still refused.
#[tokio::test]
async fn a_native_client_forging_the_pages_headers_is_refused_without_a_token() {
    let host = Harness::start().await;
    for authorization in [
        None,
        Some("Bearer not-a-token".to_owned()),
        Some(format!("Bearer {SERVER_KEY}")),
        Some(format!("Basic {SERVER_KEY}")),
        Some(format!("Bearer {}", host.ticket())),
    ] {
        let mut request = host.page_request(Method::POST, "/api/sync");
        if let Some(authorization) = &authorization {
            request = request.header("authorization", authorization);
        }
        request = request.header("x-coffret-key", SERVER_KEY);
        let response = host.send(request.body(Body::empty()).unwrap()).await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{authorization:?}"
        );
    }
    untouched(&host);
}

// LA-14, LA-18: a locked Library answers data routes `423`, and still takes
// the request that unlocks it and the page's question about its session.
#[tokio::test]
async fn while_locked_only_the_unlock_and_the_session_pass() {
    let host = Harness::start().await;
    let token = host.token().await;
    host.lock.set(true);

    for (method, path) in [
        (Method::GET, "/api/list"),
        (Method::GET, "/api/work"),
        (Method::POST, "/api/sync"),
        (Method::GET, "/api/unlock"),
    ] {
        let response = host
            .send(
                host.with_token(method.clone(), path, &token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        refused_as(response, StatusCode::LOCKED, "library_locked").await;
    }
    assert_eq!(host.upstream.connections(), 0);

    let unlock = host
        .send(
            host.with_token(Method::POST, "/api/unlock", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(unlock.status(), StatusCode::OK);

    let session = host
        .send(
            host.with_token(Method::GET, "/auth/session", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(session.status(), StatusCode::OK);
    let session: Value = json_of(session).await;
    assert_eq!(session["library"], "locked");

    host.lock.set(false);
    let read = host
        .send(
            host.with_token(Method::GET, "/api/list", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(read.status(), StatusCode::OK);
}
