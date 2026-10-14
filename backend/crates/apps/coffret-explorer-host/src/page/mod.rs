//! Every path outside `/api` and `/auth`: the explorer's page, or a word that it
//! is not here.
//!
//! Only what was compiled in is served: the explorer's build output, embedded
//! under the `embed-web` feature, and nothing from a disk at run time. A path
//! is looked up in it only once it is known to name no dotfile and to step
//! nowhere outside it ([`asset_name`]), and the build refuses an output that
//! holds a link, which would otherwise have embedded whatever it pointed at
//! (`build.rs`).

mod asset_name;
use asset_name::asset_name;

#[cfg(any(feature = "embed-web", test))]
mod assets;
#[cfg(any(feature = "embed-web", test))]
use assets::Assets;

#[cfg(any(feature = "embed-web", test))]
mod content_type;
#[cfg(any(feature = "embed-web", test))]
use content_type::content_type;

mod policy;
pub(crate) use policy::content_security_policy;

use axum::extract::Request;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};

/// The explorer's build output, compiled into the binary.
///
/// Resolved when this crate is compiled, so a build with `embed-web` needs
/// `make web-dist` to have run first, and fails rather than shipping without
/// the page.
#[cfg(feature = "embed-web")]
static BUILT: include_dir::Dir<'static> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../../../frontend/packages/apps/web/dist");

/// The page the explorer starts from, and what a path naming no file gets:
/// the explorer is a single page, and reads the path itself.
#[cfg(any(feature = "embed-web", test))]
const INDEX: &str = "index.html";

/// `Cache-Control` for the bundler's `assets/`, whose names carry a hash of
/// their contents and so never mean anything else.
#[cfg(any(feature = "embed-web", test))]
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// `Cache-Control` for everything whose name stays put when the build changes,
/// `index.html` above all: it names this build's assets.
#[cfg(any(feature = "embed-web", test))]
const NO_CACHE: &str = "no-cache";

/// The answer to a path that names a dotfile or steps outside the build.
const NOT_HERE: &str = "There is nothing here.\n";

/// Answers one request outside `/api` and `/auth`.
pub(crate) async fn serve(request: Request) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    respond(request.uri().path())
}

#[cfg(feature = "embed-web")]
fn respond(path: &str) -> Response {
    respond_from(&BUILT, path)
}

#[cfg(not(feature = "embed-web"))]
fn respond(path: &str) -> Response {
    if asset_name(path).is_none() {
        return not_here();
    }
    (
        StatusCode::NOT_FOUND,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        "The explorer was not built into this binary. Build the explorer with \
         `make web-dist`, then build this crate with its `embed-web` feature.\n",
    )
        .into_response()
}

/// Answers `path` out of `assets`: the file it names, the index for a path
/// naming none, and a fixed `404` for a path that is not a name at all.
#[cfg(any(feature = "embed-web", test))]
fn respond_from(assets: &impl Assets, path: &str) -> Response {
    use axum::http::header::CACHE_CONTROL;

    let Some(relative) = asset_name(path) else {
        return not_here();
    };
    let (name, contents) = match assets.file(&relative) {
        Some(contents) if !relative.is_empty() => (relative.as_str(), contents),
        _ => match assets.file(INDEX) {
            Some(index) => (INDEX, index),
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "The explorer built into this binary has no index.html.\n",
                )
                    .into_response()
            }
        },
    };
    let cache = if name.starts_with("assets/") {
        IMMUTABLE
    } else {
        NO_CACHE
    };
    (
        [(CONTENT_TYPE, content_type(name)), (CACHE_CONTROL, cache)],
        contents,
    )
        .into_response()
}

fn not_here() -> Response {
    (
        StatusCode::NOT_FOUND,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        NOT_HERE,
    )
        .into_response()
}

// The check `build.rs` runs over the build output, here so that the cases can
// hold it to what it finds.
#[cfg(test)]
#[path = "../../build/dist_links.rs"]
mod dist_links;

#[cfg(test)]
mod tests;
