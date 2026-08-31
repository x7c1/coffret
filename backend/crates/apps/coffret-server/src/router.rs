use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::routes;
use crate::state::ServerState;

/// Everything the browser may ask, over one open Library.
///
/// A router and not a server: what binds a socket is the binary, and what drives
/// this in a test is the service itself.
pub fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/api/library", get(routes::library))
        .route("/api/folders", get(routes::folders))
        .route("/api/list", get(routes::list))
        .route("/api/file", get(routes::file))
        .route("/api/activity", get(routes::activity))
        // The one route that is not a `GET`, because it is the one that asks the
        // server to go and do something rather than to say what it knows.
        .route("/api/fill", post(routes::fill))
        .with_state(state)
}
