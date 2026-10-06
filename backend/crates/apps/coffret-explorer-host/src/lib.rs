//! The explorer's serving side, as a router a caller binds wherever it likes.
//!
//! The explorer is a page, and a page cannot reach the server that answers with
//! the Library on its own: the server admits a caller only by a key it drew as
//! it started and wrote owner-only into the Library's directory on this device
//! (spec: LA-3), and a page in a browser cannot read a local file. Nor can the
//! key be handed to the page instead — it is never on a URL and never in a
//! cookie (spec: LA-6), and anything the page held, any other script in it could
//! read out. So the explorer is served from a host on the device, and that host
//! forwards `/api` to the server with the key attached. A process here can read
//! the file; the browser never sees what is in it.
//!
//! This is the twin of the proxy the explorer's dev server runs
//! (`frontend/packages/apps/web/vite.config.ts`), in Rust, so that running the
//! explorer needs neither node nor a checkout. What it does to a request is what
//! that proxy does, and [`router`] says each step and why.
//!
//! The page itself is compiled in under the `embed-web` feature. Without it —
//! the default, so that a build never needs the frontend built first — every
//! path outside `/api` answers a short plain page saying so.
//!
//! The explorer is deliberately not served from the server's own port: the
//! server would then be answering its own page without a key, and every request
//! has to be authorized before any route sees it (spec: LA-2).

mod config;
pub use config::Config;

mod error;
pub use error::Error;

mod forward;
mod page;

use std::sync::Arc;

use axum::routing::any;
use axum::Router;

use forward::Forwarder;

/// The router a caller binds: `/api` and everything under it forwarded to the
/// server, and every other path answered with the explorer's page.
///
/// What a forwarded request is, against the one that arrived:
///
/// - the method, the path and the query unchanged, and the body streamed rather
///   than gathered, in both directions;
/// - any key the caller sent removed, and the key in [`Config::key_file`] set in
///   its place — read afresh on every request, because a server that was
///   started again drew a new one (spec: LA-4) and this host outlives it. A file
///   that cannot be read forwards no key at all, and the server's refusal is
///   what the person sees: it is the server's to say, not this host's to invent;
/// - `Host` naming the server, which refuses any other (spec: LA-5);
/// - `Origin` rewritten to the server's only where it is this host's own, so
///   that the explorer's own requests are the server's own, and a page on any
///   other site still arrives as what it is and is refused;
/// - hop-by-hop headers dropped, in both directions; everything else, the
///   `Sec-Fetch-*` headers among them, as it came.
///
/// A redirect the server answers with is handed back rather than followed. A
/// server that is not answering is a `502` naming its address, so that the half
/// that is wrong is plain from the page.
pub fn router(config: Config) -> Result<Router, Error> {
    let forwarder = Arc::new(Forwarder::new(config).map_err(Error)?);
    Ok(Router::new()
        .route("/api", any(forward::forward))
        .route("/api/{*rest}", any(forward::forward))
        .with_state(forwarder)
        .fallback(page::serve))
}
