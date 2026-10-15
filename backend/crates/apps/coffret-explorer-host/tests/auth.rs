//! `/auth`, which the host answers itself, and what a browser token admits once
//! it has one: the exchange, the session, its revocation, the limits on what a
//! request to `/auth` may bring, and the generation a token belongs to
//! (spec: LA-14, LA-15, LA-16, LA-17, LA-18, LA-19).

mod support;

use std::io;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{Method, StatusCode};
use axum::response::Response;
use futures_util::stream;
use serde_json::Value;
use support::{json_of, Harness, SERVER_KEY};
use tokio::sync::Barrier;

async fn error_of(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    assert!(
        response
            .headers()
            .keys()
            .all(|name| !name.as_str().starts_with("access-control-")),
        "{:?}",
        response.headers()
    );
    (status, json_of(response).await["error"].clone())
}

async fn get(host: &Harness, path: &str, token: &str) -> Response {
    host.send(
        host.with_token(Method::GET, path, token)
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

// LA-16, LA-15: a ticket exchanges for a token, and the token's requests reach
// the server with the server key and never with the token.
#[tokio::test]
async fn an_exchanged_token_reaches_the_server_with_the_server_key() {
    let host = Harness::start().await;
    let response = host.send(host.launch(&host.ticket())).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let launched = json_of(response).await;
    let token = launched["token"].as_str().unwrap().to_owned();
    assert_eq!(token.len(), 43);
    assert_eq!(launched["expires_in"], 12 * 60 * 60);
    assert_eq!(
        launched["instance_id"],
        host.credentials.instance_id().to_string()
    );

    for method in [Method::GET, Method::POST] {
        let response = host
            .send(
                host.with_token(method.clone(), "/api/headers?folder=a%20b", &token)
                    .header("cookie", "a=b")
                    .header("x-coffret-key", "a-key-the-caller-made-up")
                    .header("forwarded", "for=evil")
                    .header("x-forwarded-for", "evil")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK, "{method}");
        assert_eq!(response.headers()["cache-control"], "no-store");
        let sent = json_of(response).await;
        let headers = &sent["headers"];
        assert_eq!(headers["x-coffret-key"], serde_json::json!([SERVER_KEY]));
        assert_eq!(
            headers["host"],
            serde_json::json!([host.upstream.address.to_string()])
        );
        assert_eq!(
            headers["origin"],
            serde_json::json!([format!("http://{}", host.upstream.address)])
        );
        assert_eq!(
            headers["sec-fetch-site"],
            serde_json::json!(["same-origin"])
        );
        for removed in ["authorization", "cookie", "forwarded", "x-forwarded-for"] {
            assert!(headers[removed].is_null(), "{removed} reached the server");
        }
        assert!(
            !sent.to_string().contains(&token),
            "the browser token reached the server"
        );
        assert_eq!(sent["method"], method.as_str());
        assert_eq!(sent["uri"], "/api/headers?folder=a%20b");
    }
}

// LA-15: what comes back is not made into a credential, and is not a CORS
// answer.
#[tokio::test]
async fn the_servers_cookies_and_redirects_are_not_carried_through() {
    let host = Harness::start().await;
    let token = host.token().await;

    let answer = get(&host, "/api/cookie", &token).await;
    assert_eq!(answer.status(), StatusCode::OK);
    assert!(answer.headers().get("set-cookie").is_none());
    assert!(answer
        .headers()
        .get("access-control-allow-origin")
        .is_none());
    assert_eq!(answer.headers()["x-kept"], "1");

    let redirect = get(&host, "/api/redirect", &token).await;
    assert_eq!(redirect.status(), StatusCode::SEE_OTHER);
    assert_eq!(redirect.headers()["location"], "/elsewhere");
}

// LA-17: a used, expired-by-retirement or never-issued ticket is one `401`.
#[tokio::test]
async fn a_ticket_that_is_not_good_is_one_refusal() {
    let host = Harness::start().await;

    let used = host.ticket();
    assert_eq!(host.send(host.launch(&used)).await.status(), StatusCode::OK);
    let after_restart = host.ticket();
    host.credentials.retire();

    for ticket in [used, after_restart, "A".repeat(43), "not a ticket".into()] {
        let (status, error) = error_of(host.send(host.launch(&ticket)).await).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{ticket}");
        assert_eq!(error, "authentication_required");
    }

    let no_ticket = host
        .send(
            host.page_request(Method::POST, "/auth/launch")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(no_ticket.status(), StatusCode::UNAUTHORIZED);
}

// LA-17: of concurrent exchanges of one ticket, exactly one yields a token.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_exchanges_of_one_ticket_yield_one_token() {
    const EXCHANGES: usize = 12;
    let host = Arc::new(Harness::start().await);
    let ticket = Arc::new(host.ticket());
    let start = Arc::new(Barrier::new(EXCHANGES));

    let exchanges: Vec<_> = (0..EXCHANGES)
        .map(|_| {
            let (host, ticket, start) =
                (Arc::clone(&host), Arc::clone(&ticket), Arc::clone(&start));
            tokio::spawn(async move {
                start.wait().await;
                host.send(host.launch(&ticket)).await.status()
            })
        })
        .collect();
    let mut statuses = Vec::new();
    for exchange in exchanges {
        statuses.push(exchange.await.unwrap());
    }
    assert_eq!(
        statuses.iter().filter(|s| **s == StatusCode::OK).count(),
        1,
        "{statuses:?}"
    );
    assert!(statuses
        .iter()
        .all(|s| *s == StatusCode::OK || *s == StatusCode::UNAUTHORIZED));
}

// LA-18: a token of a retired generation admits nothing, and neither does the
// server key shown in its place.
#[tokio::test]
async fn a_token_of_another_generation_is_refused() {
    let host = Harness::start().await;
    let token = host.token().await;
    assert_eq!(
        get(&host, "/api/list", &token).await.status(),
        StatusCode::OK
    );

    host.credentials.retire();
    for presented in [token.as_str(), SERVER_KEY] {
        let (status, error) = error_of(get(&host, "/api/list", presented).await).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(error, "authentication_required");
    }
    let fresh = host.token().await;
    assert_eq!(
        get(&host, "/api/list", &fresh).await.status(),
        StatusCode::OK
    );
}

// LA-15: a key that changed under a session is a server of another run, and
// every session goes rather than being forwarded with the newer key.
#[tokio::test]
async fn a_changed_key_revokes_every_session() {
    let host = Harness::start().await;
    let token = host.token().await;
    let other = host.token().await;
    assert_eq!(
        get(&host, "/api/list", &token).await.status(),
        StatusCode::OK
    );
    let before = host.credentials.instance_id();

    host.key.set(Ok("the-key-of-another-run".into()));
    let (status, error) = error_of(get(&host, "/api/headers", &token).await).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(error, "server_unavailable");

    for token in [&token, &other] {
        let (status, _) = error_of(get(&host, "/api/list", token).await).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    assert_ne!(host.credentials.instance_id(), before);

    // The next generation is issued under the key that stands now.
    let fresh = host.token().await;
    let sent = json_of(get(&host, "/api/headers", &fresh).await).await;
    assert_eq!(
        sent["headers"]["x-coffret-key"],
        serde_json::json!(["the-key-of-another-run"])
    );
}

// LA-15: a key that cannot be read fails closed, at the exchange and after.
#[tokio::test]
async fn an_unreadable_key_fails_closed() {
    let host = Harness::start().await;
    let token = host.token().await;

    host.key
        .set(Err(io::Error::new(io::ErrorKind::NotFound, "gone")));
    let (status, _) = error_of(get(&host, "/api/list", &token).await).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let (status, _) = error_of(get(&host, "/api/list", &token).await).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, error) = error_of(host.send(host.launch(&host.ticket())).await).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(error, "server_unavailable");
}

// LA-15: a server that stopped is the end of every session.
#[tokio::test]
async fn a_server_that_stopped_revokes_every_session() {
    // A port that was free a moment ago and that nothing listens on now.
    let gone = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .unwrap();
    let mut upstream = support::Upstream::start().await;
    upstream.address = gone;
    let host = Harness::in_front_of(upstream).await;
    let token = host.token().await;

    let answer = get(&host, "/api/list", &token).await;
    assert_eq!(answer.status(), StatusCode::BAD_GATEWAY);
    let body = json_of(answer).await;
    // The word that tells the page its session went with the server.
    assert_eq!(body["error"], "server_unavailable");
    let message = body["message"].as_str().expect("a sentence");
    assert!(message.contains(&gone.to_string()), "{message}");
    assert!(message.contains("not answering"), "{message}");

    let (status, _) = error_of(get(&host, "/api/list", &token).await).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// LA-16, LA-18: the session says when it ends and whether the Library is
// locked, and its own page can end it without ending any other.
#[tokio::test]
async fn a_session_is_read_and_revoked_by_its_own_page() {
    let host = Harness::start().await;
    let token = host.token().await;
    let other = host.token().await;

    let session = get(&host, "/auth/session", &token).await;
    assert_eq!(session.status(), StatusCode::OK);
    assert_eq!(session.headers()["cache-control"], "no-store");
    let session = json_of(session).await;
    assert_eq!(session["library"], "unlocked");
    assert!(session["expires_in"].as_u64().unwrap() <= 12 * 60 * 60);
    assert_eq!(session.as_object().unwrap().len(), 2, "{session}");

    let revoked = host
        .send(
            host.with_token(Method::POST, "/auth/session/revoke", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(revoked.status(), StatusCode::NO_CONTENT);
    let (status, _) = error_of(get(&host, "/auth/session", &token).await).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = error_of(get(&host, "/api/list", &token).await).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        get(&host, "/api/list", &other).await.status(),
        StatusCode::OK
    );

    let (status, _) = error_of(get(&host, "/auth/session", "").await).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// LA-16: `/auth` is the host's own, a path under it it does not answer is a
// fixed `404`, and no method a route does not take is answered, `OPTIONS`
// included.
#[tokio::test]
async fn auth_answers_its_own_routes_and_nothing_else() {
    let host = Harness::start().await;
    for path in [
        "/auth",
        "/auth/",
        "/auth/launch/x",
        "/auth/tickets",
        "/auth/../api/list",
    ] {
        let (status, error) = error_of(
            host.send(
                host.page_request(Method::GET, path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(error, "not_found");
    }
    for (method, path) in [
        (Method::GET, "/auth/launch"),
        (Method::OPTIONS, "/auth/launch"),
        (Method::OPTIONS, "/auth/session"),
        (Method::POST, "/auth/session"),
    ] {
        let (status, _) = error_of(
            host.send(
                host.page_request(method.clone(), path)
                    .header("access-control-request-method", "POST")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{method} {path}");
    }
    let elsewhere = host
        .send(
            host.request(Method::POST, "/auth/launch")
                .header("origin", "https://evil.invalid")
                .header("authorization", format!("Launch {}", host.ticket()))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(elsewhere.status(), StatusCode::FORBIDDEN);
    assert_eq!(host.upstream.connections(), 0);
}

fn launch_with(host: &Harness, content_type: &str, body: Body) -> axum::http::Request<Body> {
    host.page_request(Method::POST, "/auth/launch")
        .header("authorization", format!("Launch {}", host.ticket()))
        .header("content-type", content_type)
        .body(body)
        .unwrap()
}

// LA-16, LA-19: an exchange is JSON, within 1 KiB, and its headers within
// 8 KiB.
#[tokio::test]
async fn an_exchange_is_bounded_and_json() {
    let host = Harness::start().await;

    let (status, _) = error_of(
        host.send(launch_with(&host, "text/plain", Body::from("{}")))
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let (status, _) = error_of(
        host.send(launch_with(
            &host,
            "application/json",
            Body::from("not json"),
        ))
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let declared = host
        .page_request(Method::POST, "/auth/launch")
        .header("authorization", format!("Launch {}", host.ticket()))
        .header("content-type", "application/json")
        .header("content-length", "2048")
        .body(Body::from(vec![b' '; 2048]))
        .unwrap();
    let declared = host.send(declared).await;
    assert_eq!(declared.headers()["connection"], "close");
    let (status, error) = error_of(declared).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(error, "body_too_large");

    let undeclared = Body::from_stream(stream::iter(
        (0..4).map(|_| Ok::<_, io::Error>(Bytes::from(vec![b' '; 512]))),
    ));
    let (status, _) = error_of(
        host.send(launch_with(&host, "application/json", undeclared))
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    let headers = host
        .page_request(Method::GET, "/api/list")
        .header("x-padding", "a".repeat(9 * 1024))
        .body(Body::empty())
        .unwrap();
    let (status, error) = error_of(host.send(headers).await).await;
    assert_eq!(status, StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE);
    assert_eq!(error, "headers_too_large");
    assert_eq!(host.upstream.connections(), 0);
}

// LA-19: a request to `/auth` that keeps the host waiting is given up on.
#[tokio::test(start_paused = true)]
async fn an_exchange_that_never_finishes_arriving_is_given_up_on() {
    let host = Harness::start().await;
    let never = Body::from_stream(stream::pending::<Result<Bytes, io::Error>>());
    let (status, error) = error_of(
        host.send(launch_with(&host, "application/json", never))
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::REQUEST_TIMEOUT);
    assert_eq!(error, "request_timeout");
}

// LA-18: past the session cap an exchange still succeeds, and the session used
// least recently — here the first, which nothing used since — is revoked to
// admit it.
#[tokio::test]
async fn the_session_cap_admits_a_ninth_exchange_by_revoking_the_least_recently_used() {
    let host = Harness::start().await;
    let mut tokens = Vec::new();
    for _ in 0..9 {
        let response = host.send(host.launch(&host.ticket())).await;
        assert_eq!(response.status(), StatusCode::OK);
        tokens.push(
            json_of(response).await["token"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }

    let (status, error) = error_of(get(&host, "/api/list", &tokens[0]).await).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error, "authentication_required");
    for token in &tokens[1..] {
        assert_eq!(
            get(&host, "/api/list", token).await.status(),
            StatusCode::OK
        );
    }
}

// LA-17: past the exchange limit an exchange is `429` with when to come back.
#[tokio::test]
async fn the_exchange_limit_answers_its_refusal() {
    let host = Harness::start().await;
    // The burst is twenty; well past it, some exchange is told to wait.
    let mut limited = None;
    for _ in 0..40 {
        let response = host.send(host.launch(&"A".repeat(43))).await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            limited = Some(response);
            break;
        }
    }
    let limited = limited.expect("the exchange limit answers");
    assert_eq!(limited.headers()["retry-after"], "1");
    let (_, error) = error_of(limited).await;
    assert_eq!(error, "rate_limited");
}
