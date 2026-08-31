use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
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
        // The three that are not a `GET`, because they are the ones that ask the
        // server to go and do something rather than to say what it knows. Two of
        // them arm background work and answer at once; the third is the one route
        // that carries anything into the Library.
        .route("/api/fill", post(routes::fill))
        .route("/api/sync", post(routes::sync))
        .route(
            "/api/upload",
            // Axum's default body limit is a couple of megabytes, which is less
            // than one photograph. Nothing here is held in memory — each part
            // streams to a temporary file inside the destination directory as it
            // arrives — so a ceiling on the whole request would be a ceiling on
            // how many files a person may drop at once, expressed in bytes.
            post(routes::upload).layer(DefaultBodyLimit::disable()),
        )
        .with_state(state)
}
