//! `/api`, forwarded to the server with the key it admits callers by, once a
//! request passed every fence before it (spec: LA-14).

mod failure_kind;
use failure_kind::failure_kind;

mod headers;

mod key_pin;
use key_pin::KeyPin;
use key_pin::KeyProblem;

mod not_answering;
#[cfg(test)]
pub(crate) use not_answering::answer as not_answering_answer;
use not_answering::not_answering;

mod remove_hop_by_hop;
use remove_hop_by_hop::remove_hop_by_hop;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{ORIGIN, SET_COOKIE};
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::Response;
use futures_util::TryStreamExt;

use crate::credentials::{InstanceId, Session};
use crate::host::Host;
use crate::refusal::Refusal;
use crate::Config;

/// The names of the CORS answers, none of which the host passes on: it answers
/// no cross-origin request (spec: LA-16).
const ACCESS_CONTROL: &str = "access-control-";

pub(crate) struct Forwarder {
    client: reqwest::Client,
    server: SocketAddr,
    /// `<server>`, the only `Host` the server accepts.
    server_host: HeaderValue,
    /// `http://<server>`, which is what the server's own origin is.
    server_origin: HeaderValue,
    /// The key each generation of sessions is forwarded with.
    pin: KeyPin,
}

impl Forwarder {
    pub(crate) fn new(config: &Config) -> reqwest::Result<Self> {
        let client = reqwest::Client::builder()
            // A redirect is the server's answer, for the page to act on; one
            // followed here would be a second request the page never made
            // (spec: LA-15).
            .redirect(reqwest::redirect::Policy::none())
            // The server is on this device's loopback interface. A proxy named
            // in the environment would carry the key off it.
            .no_proxy()
            .build()?;
        let server_host = HeaderValue::from_str(&config.server.to_string())
            .expect("a socket address is a valid header value");
        let server_origin = HeaderValue::from_str(&format!("http://{}", config.server))
            .expect("a socket address is a valid header value");
        Ok(Self {
            client,
            server: config.server,
            server_host,
            server_origin,
            pin: KeyPin::new(Arc::clone(&config.key)),
        })
    }
}

/// Forwards one admitted request under `/api` to the server and hands its
/// answer back.
///
/// What a forwarded request is, against the one that arrived:
///
/// - the method, the path and the query unchanged, and the body streamed rather
///   than gathered, in both directions;
/// - `Authorization`, `Cookie`, any key, `Forwarded` and every `X-Forwarded-*`
///   the caller sent removed, and the key the session's generation was issued
///   under set in their place (spec: LA-14, LA-15);
/// - `Host` naming the server, which refuses any other (spec: LA-5);
/// - `Origin` rewritten to the server's where it was verified as this host's
///   own, and left absent where there was none;
/// - hop-by-hop headers dropped, in both directions; everything else, the
///   `Sec-Fetch-*` headers among them, as it came.
///
/// What comes back is handed back without `Set-Cookie` and without any CORS
/// header, and a redirect is handed back rather than followed (spec: LA-15,
/// LA-16). A server that is not answering is a `502` naming its address, so
/// that the half that is wrong is plain from the page; it is also a server
/// that stopped, and every session goes with it (spec: LA-15).
pub(crate) async fn forward(State(host): State<Arc<Host>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let Some(session) = parts.extensions.get::<Session>() else {
        // The fences put it there; a request without one did not pass them.
        return Refusal::AuthenticationRequired.answer("token", &parts.headers);
    };
    let forwarder = &host.forwarder;
    let key = match forwarder.pin.key_for(session.instance_id()) {
        Ok(key) => key,
        Err(problem) => return fail_closed(&host, problem, &parts.headers),
    };
    let from_own_page = parts
        .headers
        .get(ORIGIN)
        .is_some_and(|origin| host.authority.is_origin(origin));

    let path_and_query = parts
        .uri
        .path_and_query()
        .map_or("/", |path_and_query| path_and_query.as_str());
    let url = format!("http://{}{}", forwarder.server, path_and_query);
    let headers = forwarder.headers(parts.headers, key, from_own_page);

    let sent = forwarder
        .client
        .request(parts.method, url)
        .headers(headers)
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await;
    let answer = match sent {
        Ok(answer) => answer,
        Err(cause) => {
            if cause.is_connect() {
                host.gate.fail_closed("server_stopped");
            }
            return not_answering(forwarder.server, &cause);
        }
    };

    let status = answer.status();
    let mut headers = answer.headers().clone();
    remove_hop_by_hop(&mut headers);
    remove_credentials_and_cors(&mut headers);
    let server = forwarder.server;
    // A failure midway is met after the status has gone, so the browser sees
    // only a body cut short; this is the one place it can still be said. Said
    // as the kind of failure rather than as `cause`, whose text names the URL.
    let body = answer.bytes_stream().inspect_err(move |cause| {
        tracing::warn!(
            %server,
            operation = "stream_answer",
            kind = failure_kind(cause),
            "the server's answer broke off midway",
        );
    });
    let mut response = Response::new(Body::from_stream(body));
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    response
}

/// Pins the key the generation of a session just made is forwarded with
/// (spec: LA-15). Where it cannot be read, or is not the one already pinned,
/// the host fails closed, and this is the answer to give the exchange instead.
pub(crate) fn unpinned_answer(
    host: &Host,
    generation: &InstanceId,
    request: &HeaderMap,
) -> Option<Response> {
    let problem = host.forwarder.pin.pin(generation).err()?;
    Some(fail_closed(host, problem, request))
}

/// Revokes every session because the server key could not be confirmed, and
/// the answer that says so.
///
/// What is said is why, in fixed words: never where the file is, whose path
/// names the Library's directory on this device, never the read's own message,
/// which may name it too, and never the key (spec: EL-1, spec: EL-3).
fn fail_closed(host: &Host, problem: KeyProblem, request: &HeaderMap) -> Response {
    match problem.io_error_kind() {
        Some(kind) => tracing::warn!(
            problem = problem.as_str(),
            io_error_kind = ?kind,
            "the server's key could not be confirmed; every session was revoked",
        ),
        None => tracing::warn!(
            problem = problem.as_str(),
            "the server's key could not be confirmed; every session was revoked",
        ),
    }
    host.gate.fail_closed("server_key");
    Refusal::ServerUnavailable.answer("server_key", request)
}

/// Drops what would make a credential of the server's answer, or answer a
/// cross-origin request (spec: LA-15, LA-16).
fn remove_credentials_and_cors(headers: &mut HeaderMap) {
    headers.remove(SET_COOKIE);
    let cors: Vec<HeaderName> = headers
        .keys()
        .filter(|name| name.as_str().starts_with(ACCESS_CONTROL))
        .cloned()
        .collect();
    for name in cors {
        headers.remove(name);
    }
}
