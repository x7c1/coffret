//! The door's contract, as tests: each case drives the door's router over a
//! desktop of its own, as a window's requests reach it.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use coffret_explorer_host::credentials::Secret;
use coffret_logging::testing::CapturedLogs;
use serde_json::{json, Value};
use tower::ServiceExt;

use super::submission::MAX_BODY_BYTES;
use super::windows::MAX_REFUSED;
use super::*;
use crate::testing::{self, create, PASSPHRASE};
use crate::{Desktop, Event};

mod drops;
mod events;
mod fences;
mod page;
mod submissions;
mod tokens;

/// A desktop of the case's own, the door's router over it, and the door's
/// authority and origin as a window's requests spell them.
struct Case {
    desktop: Desktop,
    router: Router,
    authority: String,
    origin: String,
}

/// What the door answered.
struct Answered {
    status: StatusCode,
    headers: HeaderMap,
    text: String,
}

impl Answered {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or_else(|_| panic!("JSON: {}", self.text))
    }
}

impl Case {
    fn new() -> Self {
        let desktop = testing::desktop();
        let router = Door::new(desktop.door(), Arc::clone(desktop.shared()))
            .expect("the door is on loopback")
            .router();
        let authority = desktop.door().to_string();
        Self {
            desktop,
            router,
            origin: format!("http://{authority}"),
            authority,
        }
    }

    /// A window's token, opened in `mode`, out of the URL the shell is handed.
    fn token(&self, mode: DoorMode) -> String {
        let url = self.desktop.open_door(mode).expect("a window opens");
        let (_, token) = url
            .expose()
            .split_once("#door=")
            .expect("the URL carries the token in its fragment");
        token.to_owned()
    }

    /// A request naming the door's own authority, and nothing else.
    fn request(&self, method: Method, uri: &str) -> axum::http::request::Builder {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("host", &self.authority)
    }

    /// The submission the door's own page sends with `token`.
    fn submission(&self, token: &str, body: Value) -> Request<Body> {
        self.raw_submission(token, Body::from(body.to_string()))
    }

    /// A submission as the door's own page would send it, with whatever
    /// `body` brings in place of the JSON the page sends.
    fn raw_submission(&self, token: &str, body: Body) -> Request<Body> {
        self.request(Method::POST, "/submit")
            .header("origin", &self.origin)
            .header("sec-fetch-site", "same-origin")
            .header("authorization", format!("Door {token}"))
            .header("content-type", "application/json")
            .body(body)
            .unwrap()
    }

    /// What the door's own page sends to learn what `token`'s window is for.
    fn look(&self, token: &str) -> Request<Body> {
        self.request(Method::GET, "/window")
            .header("sec-fetch-site", "same-origin")
            .header("authorization", format!("Door {token}"))
            .body(Body::empty())
            .unwrap()
    }

    async fn send(&self, request: Request<Body>) -> Answered {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("a router never fails");
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        Answered {
            status,
            headers,
            text: String::from_utf8(bytes.to_vec()).unwrap(),
        }
    }

    /// Drives `case` on the desktop's runtime, which every task it starts
    /// runs on.
    fn run<F: std::future::Future>(&self, case: F) -> F::Output {
        self.desktop.runtime().block_on(case)
    }

    /// The events the shell is told of from now on.
    fn events(&self) -> Arc<Mutex<Vec<Event>>> {
        let told = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&told);
        self.desktop
            .listen(move |event| sink.lock().unwrap().push(event));
        told
    }
}

/// What a window submits to open the Library a case created as `library`:
/// that Library's ID, which is what the window lists it under, or `library`
/// itself where no Library carries the name — for the cases that are refused
/// before the Library is looked at.
fn opening(library: &str, passphrase: &str) -> Value {
    let id = coffret_device::LibraryDir::find(&coffret_device::LibraryRef::named(library))
        .ok()
        .and_then(|dir| coffret_device::DeviceSettings::read(&dir).ok())
        .map(|settings| settings.library_id.to_hex())
        .unwrap_or_else(|| library.to_owned());
    json!({ "library": id, "passphrase": passphrase })
}

fn assert_refused(answered: &Answered, status: StatusCode, error: &str) {
    assert_eq!(answered.status, status, "{}", answered.text);
    assert_eq!(answered.json()["error"], error, "{}", answered.text);
}

/// `POST /auth/launch` at the explorer's host with `ticket`, as the explorer's
/// page sends it.
async fn exchange(explorer: SocketAddr, ticket: &str) -> (reqwest::StatusCode, Value) {
    let answer = reqwest::Client::new()
        .post(format!("http://{explorer}/auth/launch"))
        .header("origin", format!("http://{explorer}"))
        .header("sec-fetch-site", "same-origin")
        .header("authorization", format!("Launch {ticket}"))
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("the explorer's host answers");
    let status = answer.status();
    let body = answer.bytes().await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}
