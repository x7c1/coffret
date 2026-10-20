use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_SECURITY_POLICY, ORIGIN};
use axum::http::header::{REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS};
use axum::http::{HeaderMap, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

use super::{Door, Refusal};

/// How many bytes of headers any request to the door may bring, as for the
/// explorer's host (spec: LA-19).
const MAX_HEADER_BYTES: usize = 8 * 1024;

/// The header a browser says where a request came from in.
const SEC_FETCH_SITE: &str = "sec-fetch-site";

/// The scheme a window's token is presented under.
const DOOR_SCHEME: &str = "Door ";

/// What the door's page may load and do: its own script and style, requests to
/// its own origin, and nothing else, framed by no page.
const CONTENT_SECURITY_POLICY_VALUE: &str = "default-src 'none'; script-src 'self'; \
     style-src 'self'; connect-src 'self'; img-src 'none'; form-action 'none'; \
     frame-ancestors 'none'; base-uri 'none'";

/// Refuses a request whose headers are past their bound, or whose `Host` is
/// not the authority the door's listener bound: one `Host`, exactly that one
/// (spec: LA-13, LA-22).
///
/// Over every path, the page's included: a page served under somebody else's
/// name is that name's page as far as the browser is concerned.
pub(super) async fn fixed_host(
    State(door): State<Arc<Door>>,
    request: Request,
    next: Next,
) -> Response {
    if header_bytes(request.headers()) > MAX_HEADER_BYTES {
        return Refusal::HeadersTooLarge.answer("header_bytes", false);
    }
    if !door.authority.is_named_by(request.headers(), request.uri()) {
        return Refusal::OriginRefused.answer("host", false);
    }
    next.run(request).await
}

/// Puts the headers every answer of the door carries on it, whatever
/// answered: no referrer, no sniffing, nothing stored, the page's content
/// security policy, and no framing. No `Access-Control-Allow-*` header is ever
/// sent: the door answers its own page and no other origin (spec: LA-22).
pub(super) async fn answer_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY_VALUE),
    );
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}

impl Door {
    /// Whether what the browser says about a submission admits it as one the
    /// door's own page made: an `Origin` that is the door's own exactly, and
    /// `Sec-Fetch-Site: same-origin`, each given once (spec: LA-22).
    ///
    /// Both are required, unlike the explorer host's fences, which take a
    /// request carrying neither: the one client the door answers is the
    /// shell's own window, a Chromium that sends both on every `POST`.
    pub(super) fn admits_submission(&self, headers: &HeaderMap) -> Result<(), &'static str> {
        match exactly_one(headers, ORIGIN.as_str()) {
            Some(origin) if self.authority.is_origin(origin) => {}
            _ => return Err("origin"),
        }
        same_origin_fetch(headers)
    }

    /// Whether what the browser says about a look at the window's mode admits
    /// it: `Sec-Fetch-Site: same-origin`, and an `Origin`, where there is one,
    /// that is the door's own (spec: LA-22). A same-origin `GET` carries no
    /// `Origin`.
    pub(super) fn admits_look(&self, headers: &HeaderMap) -> Result<(), &'static str> {
        let mut origins = headers.get_all(ORIGIN).iter();
        match (origins.next(), origins.next()) {
            (None, _) => {}
            (Some(origin), None) if self.authority.is_origin(origin) => {}
            _ => return Err("origin"),
        }
        same_origin_fetch(headers)
    }
}

/// `Ok` where `Sec-Fetch-Site` is given once, as `same-origin`.
fn same_origin_fetch(headers: &HeaderMap) -> Result<(), &'static str> {
    match exactly_one(headers, SEC_FETCH_SITE) {
        Some(site) if site.as_bytes() == b"same-origin" => Ok(()),
        _ => Err("fetch_site"),
    }
}

/// The token a request presents, from `Authorization: Door <token>` given
/// once; `None` for a request presenting none, or more than one.
pub(super) fn presented_token(headers: &HeaderMap) -> Option<&str> {
    exactly_one(headers, AUTHORIZATION.as_str())?
        .to_str()
        .ok()?
        .strip_prefix(DOOR_SCHEME)
}

/// The one value of `name`, and `None` where there is none or more than one.
fn exactly_one<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a HeaderValue> {
    let mut values = headers.get_all(name).iter();
    match (values.next(), values.next()) {
        (Some(value), None) => Some(value),
        _ => None,
    }
}

/// The headers' size as they were written: each name, `: `, value and line end.
fn header_bytes(headers: &HeaderMap) -> usize {
    headers
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len() + 4)
        .sum()
}
