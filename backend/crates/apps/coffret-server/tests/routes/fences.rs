//! Who is answered at all (spec: LA-2, LA-5).
//!
//! The other flows are about what a route says to the explorer on this device.
//! These are about the fences in front of every one of them, which are what
//! makes "the explorer on this device" mean anything.

use axum::body::Body;
use axum::http::{HeaderName, Request};
use coffret_logging::testing::CapturedLogs;
use coffret_server::SERVER_KEY_HEADER;
use tracing::Level;

use crate::support;
use crate::support::{asking, json as body_of, route, Served, SERVER_KEY};

/// Every route this server has, as a caller reaches it.
///
/// Written out rather than derived, because what the case over it says is that
/// there is no route without the fences — and a list the router generated would
/// hold whatever the router holds.
const EVERY_ROUTE: [(&str, &str); 12] = [
    ("GET", "/api/library"),
    ("GET", "/api/folders"),
    ("GET", "/api/list"),
    ("GET", "/api/file?path=albums/cover.png"),
    ("GET", "/api/work"),
    ("POST", "/api/fill?path=albums"),
    ("POST", "/api/sync"),
    ("POST", "/api/freeze?path=albums"),
    ("POST", "/api/refresh"),
    ("POST", "/api/reconnect"),
    ("POST", "/api/upload?path=albums"),
    ("POST", "/api/upload?path=albums&freeze=true"),
];

/// The same request the explorer would send, with one header put otherwise.
///
/// Replaced rather than added to, and removed where the case says nothing:
/// [`asking`] has already put the address and the key on the request, and a
/// second value under one name would leave the first one standing.
fn instead(header: &str, value: Option<&str>, method: &str, uri: &str) -> Request<Body> {
    let mut builder = asking(method, uri);
    let headers = builder
        .headers_mut()
        .expect("the request is well formed so far");
    let name = HeaderName::from_bytes(header.as_bytes()).expect("a case names real headers");
    match value {
        Some(value) => {
            headers.insert(name, value.parse().expect("a case sends text"));
        }
        None => {
            headers.remove(name);
        }
    }
    builder
        .body(Body::empty())
        .expect("a request with no body is well formed")
}

// A path this server registers nothing at — the old spike's listing route here
// — and a registered path asked by a method it does not take are both this
// server answering, so both come back in the one shape a refusal takes rather
// than as an empty body a page would read as somebody else replying. One kind,
// at the two statuses that tell them apart, and neither sentence repeats what
// was asked.
#[tokio::test]
async fn what_no_route_answers_is_still_a_refusal_of_the_one_shape() {
    let served = Served::library().await;

    for (method, uri, expected) in [
        ("GET", "/api/entries", 404),
        ("POST", "/api/file?path=albums/cover.png", 405),
        ("GET", "/api/sync", 405),
    ] {
        let (status, refusal) = route(&served, method, uri).await;
        assert_eq!(status, expected, "{method} {uri}");
        assert_eq!(refusal["error"], "no_such_route", "{method} {uri}");
        let said = refusal["message"]
            .as_str()
            .expect("a refusal carries one sentence");
        assert!(!said.is_empty(), "{method} {uri}");
        for echoed in ["entries", "file", "sync", "GET", "POST"] {
            assert!(
                !said.contains(echoed),
                "{method} {uri} echoed the request: {said}"
            );
        }
    }
}

// The fences stand in front of what no route answers too: a caller with no key
// learns nothing about which paths this server has.
#[tokio::test]
async fn what_no_route_answers_is_behind_the_fences() {
    let served = Served::library().await;

    let asked = instead(SERVER_KEY_HEADER, None, "GET", "/api/entries");
    let (status, refusal) = body_of(served.send(asked).await).await;
    assert_eq!(status, 403);
    assert_eq!(refusal["error"], "unauthorized");
}

// Reads and mutations alike, and no exception for the ones that only say what
// the server knows: an Entry Path is the person's own name for their file, and
// the file route answers with plaintext.
#[tokio::test]
async fn no_route_answers_a_request_that_shows_no_key() {
    let served = Served::library().await;

    for (method, uri) in EVERY_ROUTE {
        let asked = instead(SERVER_KEY_HEADER, None, method, uri);
        let (status, refusal) = body_of(served.send(asked).await).await;
        assert_eq!(status, 403, "{method} {uri} answered without a key");
        assert_eq!(refusal["error"], "unauthorized", "{method} {uri}");
    }
}

// A key of the right shape and the wrong value, on a read and on a mutation.
// Answered exactly as no key at all, so that a caller guessing learns nothing
// from the difference between the two.
#[tokio::test]
async fn a_key_that_is_not_this_server_s_is_refused() {
    let served = Served::library().await;
    let wrong = "0000000000000000000000000000000000000000000000000000000000000000";

    for (method, uri) in [("GET", "/api/list?path=albums"), ("POST", "/api/refresh")] {
        let asked = instead(SERVER_KEY_HEADER, Some(wrong), method, uri);
        let (status, refusal) = body_of(served.send(asked).await).await;
        assert_eq!(status, 403, "{method} {uri} answered a key it never drew");
        assert_eq!(refusal["error"], "unauthorized", "{method} {uri}");
    }

    // And the same two, shown the key this server did draw.
    let (status, _) = body_of(served.get("/api/list?path=albums").await).await;
    assert_eq!(status, 200);
    let (status, _) = body_of(served.post("/api/refresh").await).await;
    assert_eq!(status, 200);
}

// A hostname somebody else's DNS pointed at this socket arrives carrying that
// name, and nothing about the connection tells it from the explorer's request.
// The `Host` does, and it is read before the key: a request that reached here by
// another name is refused whatever else it carries.
#[tokio::test]
async fn a_host_naming_somewhere_else_is_refused_holding_the_key() {
    let served = Served::library().await;

    let asked = instead(
        "host",
        Some("coffret.example.com:8787"),
        "GET",
        "/api/list?path=albums",
    );

    let (status, refusal) = body_of(served.send(asked).await).await;
    assert_eq!(status, 403);
    assert_eq!(refusal["error"], "unauthorized");
}

// LA-5. The sentence that refusal is displayed with names the address this
// server really bound, which is the one thing that makes it actionable: the
// caller has already reached that address, so nothing is given away, and a
// person looking at a hostname of their own in a browser bar has no other way
// to see what to put there instead.
#[tokio::test]
async fn a_host_refusal_names_the_address_this_server_bound() {
    let served = Served::library().await;

    let asked = instead(
        "host",
        Some("coffret.example.com:8787"),
        "GET",
        "/api/list?path=albums",
    );

    let (status, refusal) = body_of(served.send(asked).await).await;
    assert_eq!(status, 403);
    let said = refusal["message"]
        .as_str()
        .expect("a refusal carries a sentence");
    assert!(said.contains(support::AUTHORITY), "{said}");
    // And not the name the request asked for, which is the caller's own text.
    assert!(!said.contains("coffret.example.com"), "{said}");
}

// LA-5's third fence. `Origin` and `Sec-Fetch-Site` are the browser's own
// account of where a request came from and a page cannot forge either, so a
// page on another site is refused even in the state where it somehow holds a
// key.
#[tokio::test]
async fn a_page_on_another_site_is_refused_holding_the_key() {
    let served = Served::library().await;

    for (header, value) in [
        ("origin", "https://elsewhere.example"),
        ("sec-fetch-site", "cross-site"),
    ] {
        let asked = instead(header, Some(value), "POST", "/api/sync");
        let (status, refusal) = body_of(served.send(asked).await).await;
        assert_eq!(status, 403, "a {header} of {value} was answered");
        assert_eq!(refusal["error"], "unauthorized", "{header}: {value}");
    }
}

// The one thing a refusal must not do is help. Neither the body nor the log says
// what the key is — the body is displayed verbatim on a screen and the log is
// read somewhere the request never was — and neither says what was shown either,
// which is a caller's own text.
#[tokio::test]
async fn a_refusal_never_says_what_the_key_is() {
    let served = Served::library().await;
    let logs = CapturedLogs::capture();

    let guessed = "8f14e45fceea167a5a36dedd4bea2543a1b2c3d4e5f60718293a4b5c6d7e8f91";
    let asked = instead(
        SERVER_KEY_HEADER,
        Some(guessed),
        "GET",
        "/api/list?path=albums",
    );

    let (status, refusal) = body_of(served.send(asked).await).await;
    assert_eq!(status, 403);
    let said = refusal.to_string();
    assert!(
        !said.contains(SERVER_KEY),
        "the refusal echoed the key: {said}"
    );
    assert!(
        !said.contains(guessed),
        "the refusal echoed the guess: {said}"
    );

    // Something was written down — a refusal nobody could find out about would
    // be worse than one that said too much — and it names the fence and nothing
    // the request carried.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "admit");
    assert_eq!(event.field("refused"), "key");
    logs.assert_free_of(&[SERVER_KEY, guessed]);
}
