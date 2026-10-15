//! `/auth`, which the host answers itself and never forwards (spec: LA-16).
//!
//! Three routes and a fixed `404` for every other path under it. No route
//! sends a CORS header, and a method a route does not take — `OPTIONS` among
//! them — is a fixed `405`.

mod launch;
mod session;

mod bounded_body;
use bounded_body::bounded_body;

use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::from_fn;
use axum::response::Response;
use axum::routing::{any, get, post};
use axum::Router;

use crate::host::Host;
use crate::refusal::Refusal;

/// How many bytes of body a request to `/auth` may bring (spec: LA-19).
pub(crate) const MAX_BODY_BYTES: usize = 1024;

/// The routes under `/auth`, each behind the bound on what it may bring.
pub(crate) fn routes() -> Router<Arc<Host>> {
    Router::new()
        .route("/auth/launch", post(launch::launch))
        .route("/auth/session", get(session::session))
        .route("/auth/session/revoke", post(session::revoke))
        .route("/auth", any(not_found))
        .route("/auth/", any(not_found))
        .route("/auth/{*rest}", any(not_found))
        .method_not_allowed_fallback(method_not_allowed)
        .layer(from_fn(bounded_body))
}

async fn not_found(request: Request) -> Response {
    Refusal::NotFound.answer("auth_path", request.headers())
}

async fn method_not_allowed(request: Request) -> Response {
    Refusal::MethodNotAllowed.answer("method", request.headers())
}
