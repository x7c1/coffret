use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::credentials::Session;
use crate::fences::bearer;
use crate::host::Host;
use crate::refusal::Refusal;

/// What `GET /auth/session` answers, and nothing else (spec: LA-16).
#[derive(Serialize)]
struct SessionAnswer {
    /// Seconds until the session expires.
    expires_in: u64,
    /// `locked` or `unlocked`.
    library: &'static str,
}

/// `GET /auth/session`: when the token's session expires, and whether the
/// Library is locked. Answered while it is locked, so that a page can say so
/// (spec: LA-14, LA-16).
pub(super) async fn session(State(host): State<Arc<Host>>, request: Request) -> Response {
    let Some(session) = authenticated(&host, &request) else {
        return Refusal::AuthenticationRequired.answer("token", request.headers());
    };
    Json(SessionAnswer {
        expires_in: session.expires_in().as_secs(),
        library: if host.lock.is_locked() {
            "locked"
        } else {
            "unlocked"
        },
    })
    .into_response()
}

/// `POST /auth/session/revoke`: ends the token's own session and no other
/// (spec: LA-16, LA-18).
pub(super) async fn revoke(State(host): State<Arc<Host>>, request: Request) -> Response {
    let Some(session) = authenticated(&host, &request) else {
        return Refusal::AuthenticationRequired.answer("token", request.headers());
    };
    host.gate.revoke(&session);
    StatusCode::NO_CONTENT.into_response()
}

/// The session the request's browser token stands for, and `None` for every
/// token that admits nobody, however it fails to (spec: LA-14).
fn authenticated(host: &Host, request: &Request) -> Option<Session> {
    bearer(request.headers()).and_then(|token| host.gate.authenticate(token).ok())
}
