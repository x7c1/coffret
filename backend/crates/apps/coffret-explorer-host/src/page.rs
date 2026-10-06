//! Every path outside `/api`: the explorer's page, or a word that it is not here.

use axum::extract::Request;
use axum::http::header::{CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
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
#[cfg(feature = "embed-web")]
const INDEX: &str = "index.html";

/// `Cache-Control` for the bundler's `assets/`, whose names carry a hash of
/// their contents and so never mean anything else.
#[cfg(feature = "embed-web")]
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// `Cache-Control` for everything whose name stays put when the build changes,
/// `index.html` above all: it names this build's assets.
#[cfg(feature = "embed-web")]
const NO_CACHE: &str = "no-cache";

/// Answers one request outside `/api`.
pub(crate) async fn serve(request: Request) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    respond(request.uri().path())
}

#[cfg(feature = "embed-web")]
fn respond(path: &str) -> Response {
    use axum::http::header::CACHE_CONTROL;

    let relative = path.trim_start_matches('/');
    let (name, file) = match BUILT.get_file(relative) {
        Some(file) if !relative.is_empty() => (relative, file),
        _ => match BUILT.get_file(INDEX) {
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
        [
            (CONTENT_TYPE, content_type(name)),
            (CACHE_CONTROL, cache),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        file.contents(),
    )
        .into_response()
}

#[cfg(not(feature = "embed-web"))]
fn respond(_path: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        [
            (CONTENT_TYPE, "text/plain; charset=utf-8"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        "The explorer was not built into this binary. Build the explorer with \
         `make web-dist`, then build this crate with its `embed-web` feature.\n",
    )
        .into_response()
}

/// The content type a file is served as, by its extension: the kinds a built
/// explorer is made of, and bytes for anything else.
#[cfg(any(feature = "embed-web", test))]
fn content_type(name: &str) -> &'static str {
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    match extension.to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::content_type;

    #[test]
    fn a_file_is_served_as_its_extension_says() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            content_type("assets/index-3f2a.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type("assets/index-3f2a.CSS"),
            "text/css; charset=utf-8"
        );
        assert_eq!(content_type("LICENSE"), "application/octet-stream");
    }
}
