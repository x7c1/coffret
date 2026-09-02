use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;

use crate::authorize::{admit, Admission};
use crate::routes;
use crate::state::ServerState;

/// Everything the browser may ask, over one open Library.
///
/// A router and not a server: what binds a socket is the binary, and what drives
/// this in a test is the service itself.
///
/// Every route is behind the same [`Admission`], layered over the whole of it
/// rather than named on each: a route added without it would be a route that
/// answers anybody, and this way there is nowhere to forget it.
///
/// Nothing here says anything about the idle lock, and that is deliberate. What
/// keeps this server awake is somebody wanting the Library, not a request
/// arriving, so the mark is made where the keys are taken —
/// `ServerState::unlocked` — rather than by a layer that would have to be told
/// which of these routes count (spec: DK-4).
pub fn router(state: Arc<ServerState>, admission: Arc<Admission>) -> Router {
    // Read here rather than reached for inside the route, because a body limit is
    // a layer a route is mounted with: by the time a handler runs, the bytes it
    // is about have already been read or refused.
    let envelope = state.envelope;
    Router::new()
        .route("/api/library", get(routes::library))
        .route("/api/folders", get(routes::folders))
        .route("/api/list", get(routes::list))
        .route("/api/file", get(routes::file))
        .route("/api/activity", get(routes::activity))
        // The six that are not a `GET`, because they are the ones that ask the
        // server to go and do something rather than to say what it knows. Three
        // of them arm background work and answer at once; the refresh does its
        // work while the request is open, because what it answers with is what
        // that work found; the upload is the one route that carries anything
        // into the Library; and the lock is the one that ends the reading of it
        // (spec: DK-3), inside this same fence because shutting somebody's
        // Library is a thing done to it.
        .route("/api/fill", post(routes::fill))
        .route("/api/lock", post(routes::lock))
        .route("/api/sync", post(routes::sync))
        .route("/api/freeze", post(routes::freeze))
        .route("/api/refresh", post(routes::refresh))
        .route(
            "/api/upload",
            // Axum's own default is a couple of megabytes, which is less than one
            // photograph, and turning it off outright would leave the one route
            // that carries bytes into the Library with no bound on a request at
            // all. Nothing here is held in memory, so the ceiling is not about
            // memory: it is about a socket that can write to this device's disk
            // for as long as somebody keeps sending.
            //
            // This is the one of the envelope's budgets that has to be a layer:
            // it counts the bytes as they arrive, so a request past it stops
            // mid-stream rather than after the route has read all of it. The
            // other two the route keeps itself.
            post(routes::upload).layer(DefaultBodyLimit::max(envelope.request_bytes)),
        )
        // Outside every route, so that a request is admitted or refused before
        // any of them has done anything at all.
        .layer(axum::middleware::from_fn_with_state(admission, admit))
        .with_state(state)
}
