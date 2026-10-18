//! Every path outside `/api` and `/auth`: the explorer's page, or a word that it
//! is not here.
//!
//! What is served is what [`PageSource`] names: the explorer's build output
//! compiled in under the `embed-web` feature, or — for a development host — the
//! same output in a directory on disk. Either way a path is looked up only once
//! it is known to name no dotfile and to step nowhere outside the build
//! ([`asset_name`]). The build refuses an output that holds a link, which would
//! otherwise have embedded whatever it pointed at (`build.rs`); a directory on
//! disk refuses a link where a request meets one ([`DistDir`]).
//!
//! A development host answers the favicon's paths with the badged favicon
//! from the same build ([`favicon`]); every other path is served the same
//! whichever host serves it.

mod asset_name;
use asset_name::asset_name;

mod assets;
use assets::Assets;

mod content_type;
use content_type::content_type;

mod dist_dir;
use dist_dir::DistDir;

mod favicon;

mod lookup;
use lookup::Lookup;

mod policy;

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::host::Host;
use crate::{HostKind, PageSource};

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
const INDEX: &str = "index.html";

/// `Cache-Control` for the bundler's `assets/`, whose names carry a hash of
/// their contents and so never mean anything else.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// `Cache-Control` for everything whose name stays put when the build changes,
/// `index.html` above all: it names this build's assets.
const NO_CACHE: &str = "no-cache";

/// The answer to a path that names a dotfile or steps outside the build.
const NOT_HERE: &str = "There is nothing here.\n";

/// The page the host serves, from where its [`PageSource`] said, as a host of
/// its [`HostKind`].
pub(crate) struct Page {
    build: Build,
    kind: HostKind,
}

/// Where the build being served is.
enum Build {
    Embedded,
    Directory(DistDir),
}

impl Page {
    pub(crate) fn new(source: &PageSource, kind: HostKind) -> Self {
        let build = match source {
            PageSource::Embedded => Build::Embedded,
            PageSource::Directory(root) => Build::Directory(DistDir::new(root.clone())),
        };
        Self { build, kind }
    }

    /// Answers `path`.
    fn respond(&self, path: &str) -> Response {
        match &self.build {
            Build::Embedded => respond_embedded(path, self.kind),
            Build::Directory(dist) => respond_from(dist, path, self.kind),
        }
    }

    /// The `Content-Security-Policy` the page and its assets are served with,
    /// for the build being served now.
    ///
    /// Read off the build's `index.html` each time for a build on disk, whose
    /// inline styles change as it is rebuilt.
    pub(crate) fn content_security_policy(&self) -> HeaderValue {
        match &self.build {
            Build::Embedded => HeaderValue::from_static(policy::embedded()),
            Build::Directory(dist) => {
                let index = std::fs::read(dist.root().join(INDEX)).ok();
                HeaderValue::from_str(&policy::policy_for(index.as_deref()))
                    .expect("the policy is ASCII")
            }
        }
    }
}

/// Answers one request outside `/api` and `/auth`.
pub(crate) async fn serve(State(host): State<Arc<Host>>, request: Request) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    host.page.respond(request.uri().path())
}

#[cfg(feature = "embed-web")]
fn respond_embedded(path: &str, kind: HostKind) -> Response {
    respond_from(&BUILT, path, kind)
}

#[cfg(not(feature = "embed-web"))]
fn respond_embedded(path: &str, _kind: HostKind) -> Response {
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

/// Answers `path` out of `assets` as a host of `kind`: the file it names (or,
/// for a favicon's name on a development host, the badged one, with the
/// content type and cache header of the name asked for), the index for a path
/// naming none, and a fixed `404` for a path that is not a name at all or that
/// reaches outside the build.
fn respond_from(assets: &impl Assets, path: &str, kind: HostKind) -> Response {
    let Some(relative) = asset_name(path) else {
        return not_here();
    };
    let (name, contents) = match assets.file(favicon::file_for(kind, &relative)) {
        Lookup::Found(contents) => (relative.as_str(), contents),
        Lookup::Refused => return not_here(),
        Lookup::Missing => match assets.file(INDEX) {
            Lookup::Found(index) => (INDEX, index),
            Lookup::Missing | Lookup::Refused => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "The explorer's build has no index.html. Build the explorer with \
                     `make web-dist`, or wait for the build that is running to finish.\n",
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
