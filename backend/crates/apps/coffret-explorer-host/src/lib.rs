//! The explorer's serving side: a router built around a socket its caller
//! bound, and the loop that answers on it.
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
//! Reaching this host is no more authorization than reaching the server is
//! (spec: LA-2). A browser is admitted by a browser token of the host's own,
//! never by the server key, and [`credentials`] holds those tokens and the
//! one-time launch tickets they are exchanged for (spec: LA-14, LA-17, LA-18).
//! What a request passes before anything is done with it is [`router`]'s to
//! say.
//!
//! The page itself is compiled in under the `embed-web` feature. Without it —
//! the default, so that a build never needs the frontend built first — every
//! page path answers a short plain page saying so. A development host serves
//! it from the build output on disk instead ([`PageSource`]).
//!
//! The ticket a browser exchanges reaches it through a file only the owner's
//! account can read, which [`launch_file`] writes (spec: LA-21).
//!
//! The explorer is deliberately not served from the server's own port: the
//! server would then be answering its own page without a key, and every request
//! has to be authorized before any route sees it (spec: LA-2).

mod answer_headers;
mod auth;
mod authority;

mod config;
pub use config::Config;

pub mod credentials;

mod error;
pub use error::Error;

mod fences;
mod forward;
mod host;

mod host_kind;
pub use host_kind::HostKind;

mod key_file;
pub use key_file::KeyFile;

mod key_source;
pub use key_source::KeySource;

#[cfg(unix)]
pub mod launch_file;

mod library_lock;
pub use library_lock::LibraryLock;

mod page;
use page::Page;

mod page_source;
pub use page_source::PageSource;

mod refusal;

mod serve;
pub use serve::serve;

use std::sync::Arc;
use std::time::Duration;

use axum::middleware::from_fn_with_state;
use axum::routing::any;
use axum::Router;
use tokio::net::TcpListener;

use authority::Authority;
use credentials::Gate;
use forward::Forwarder;
use host::Host;

/// How long reading a request to `/auth` may take, and reading any request's
/// head (spec: LA-19).
pub(crate) const READ_DEADLINE: Duration = Duration::from_secs(5);

/// The router for a host whose listener is `listener`, standing in front of
/// what `config` names and admitting browsers through `gate`.
///
/// Built from the address `listener` actually bound, which has to be
/// `127.0.0.1`, and from nothing a request says: that address is the only
/// `Host` the router answers and its origin the only `Origin` (spec: LA-13).
/// Taking the listener rather than an address is what keeps a caller from
/// building a router around an address nothing bound; serve it on that same
/// listener with [`serve`].
///
/// What a request passes, in this order, before anything is done with it:
///
/// 1. on every path, headers within their bound and the fixed `Host`;
/// 2. under `/api` and `/auth`, an `Origin` and a `Sec-Fetch-Site`, where a
///    browser sent them, that say the host's own page made it;
/// 3. under `/api`, a browser token of the current generation, and a Library
///    that is unlocked or a request that unlocks it.
///
/// Only then is a request under `/api` forwarded (spec: LA-14). `/auth` is
/// answered here and never forwarded (spec: LA-16), and every other path is
/// the explorer's page.
pub fn router(listener: &TcpListener, config: Config, gate: Gate) -> Result<Router, Error> {
    let authority = Authority::of(listener.local_addr().map_err(Error::Unbound)?)?;
    let forwarder = Forwarder::new(&config).map_err(Error::Client)?;
    let host = Arc::new(Host {
        authority,
        gate,
        lock: config.lock,
        forwarder,
        page: Page::new(&config.page, config.kind),
    });

    // `/api/` as well as the wildcard, which matches no empty rest: every path
    // under `/api` is behind the fences, one no route answers included.
    let api = Router::new()
        .route("/api", any(forward::forward))
        .route("/api/", any(forward::forward))
        .route("/api/{*rest}", any(forward::forward))
        .layer(from_fn_with_state(Arc::clone(&host), fences::admit));
    Ok(api
        .merge(auth::routes())
        .layer(from_fn_with_state(Arc::clone(&host), fences::same_origin))
        .fallback(page::serve)
        .layer(from_fn_with_state(Arc::clone(&host), fences::fixed_host))
        .layer(from_fn_with_state(
            Arc::clone(&host),
            answer_headers::answer_headers,
        ))
        .with_state(host))
}
