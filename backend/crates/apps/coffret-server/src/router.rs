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
    let allowance = state.allowance;
    Router::new()
        .route("/api/library", get(routes::library))
        .route("/api/folders", get(routes::folders))
        .route("/api/list", get(routes::list))
        .route("/api/file", get(routes::file))
        .route("/api/work", get(routes::work))
        .route("/api/browse", get(routes::browse))
        // The nine that are not a `GET`, because they are the ones that ask the
        // server to go and do something rather than to say what it knows. Four
        // of them arm background work and answer at once — the fill, the sync,
        // the freeze and the deletion; the refresh does its work while the
        // request is open, because what it answers with is what that work
        // found; the reconnect starts a consent flow and answers with the page
        // to open; the unlock asks the app's own window for the Passphrase and
        // carries none; the map records which folder on this device holds part
        // of the Library; and the upload is the one route that carries anything
        // into the Library.
        .route("/api/fill", post(routes::fill))
        .route("/api/sync", post(routes::sync))
        // The freeze is also read: a `GET` of it counts what the `POST` would
        // pack, which the explorer asks before it offers to arm one.
        .route(
            "/api/freeze",
            post(routes::freeze).get(routes::preview_freeze),
        )
        // And so is the deletion, for the same reason: a `GET` of it counts what
        // the `POST` would take out of the Library and rebuild, which the
        // explorer shows before it offers Delete.
        .route(
            "/api/delete",
            post(routes::delete).get(routes::preview_delete),
        )
        .route("/api/refresh", post(routes::refresh))
        .route("/api/reconnect", post(routes::reconnect))
        .route("/api/unlock", post(routes::unlock))
        .route("/api/map", post(routes::map))
        .route(
            "/api/upload",
            // Axum's own default is a couple of megabytes, which is less than one
            // photograph, and turning it off outright would leave the one route
            // that carries bytes into the Library with no bound on a request at
            // all. Nothing here is held in memory, so the ceiling is not about
            // memory: it is about a socket that can write to this device's disk
            // for as long as somebody keeps sending.
            //
            // This is the one of the allowance's budgets that has to be a layer
            // (spec: LA-9, LA-10): it counts the bytes as they arrive, so a
            // request past it stops mid-stream rather than after the route has
            // read all of it. The other two the route keeps itself.
            post(routes::upload).layer(DefaultBodyLimit::max(allowance.request_bytes)),
        )
        // What is asked of none of the routes above is still answered in the one
        // shape a refusal takes, rather than by axum's empty-bodied `404` and
        // `405` — answers no code here writes, which a page can only read as
        // something other than this server having replied. The second applies
        // to the routes registered before it, which is why it comes after all of
        // them.
        .fallback(routes::no_such_route)
        .method_not_allowed_fallback(routes::no_such_method)
        // Outside every route, so that a request is admitted or refused before
        // any of them has done anything at all — and outside the two answers
        // for what is no route, so that a caller without the key is told
        // nothing about which paths this server has.
        .layer(axum::middleware::from_fn_with_state(admission, admit))
        .with_state(state)
}
