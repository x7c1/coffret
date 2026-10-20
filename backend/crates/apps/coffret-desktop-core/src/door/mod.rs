//! The Passphrase door: a small HTTP listener of the core's own, on
//! `127.0.0.1`, that serves the Passphrase page and takes what it submits, so
//! that the Passphrase goes from the window's page to the core without
//! passing through the shell's own process (spec: LA-22, DK-10).
//!
//! The shell opens a window onto the door for a mode ([`DoorMode`]) and loads
//! the [`DoorUrl`] it is handed, which carries the window's token in its
//! fragment. The page takes the token out of the address, asks the door what
//! the window is for, and submits the Passphrase to the same origin it was
//! served from, so no cross-origin request is ever made or answered.
//!
//! What a request passes is the explorer host's fences, held more strictly:
//! the fixed `Host` on every path ([`fences::fixed_host`]); on a submission an
//! `Origin` that is the door's own and `Sec-Fetch-Site: same-origin`, both
//! required; a token the door holds ([`Windows`]); and a bounded body
//! ([`Submission`]). The token is the explorer host's own secret type, held as
//! its digest and compared in constant time (spec: LA-23); it ends when its
//! window's submission lands, and after five refused ones (spec: LA-24).
//!
//! No event recorded here carries a token, the door's URL or its address
//! (spec: EL-1, LA-25).

mod describe;

mod door_mode;
pub use door_mode::DoorMode;

mod door_url;
pub use door_url::DoorUrl;

mod fences;

mod page;

mod refusal;
use refusal::Refusal;

mod submission;
use submission::Submission;

mod submit;

mod windows;
pub(crate) use windows::Windows;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::{get, post};
use axum::Router;
use coffret_explorer_host::Authority;

use crate::shared::Shared;

/// What the door's routes are given: its own authority, and what the core
/// shares with them.
pub(crate) struct Door {
    authority: Authority,
    shared: Arc<Shared>,
}

impl Door {
    /// The door of a listener bound to `bound`, which has to be IPv4 loopback
    /// (spec: LA-13).
    pub(crate) fn new(
        bound: SocketAddr,
        shared: Arc<Shared>,
    ) -> Result<Self, coffret_explorer_host::Error> {
        Ok(Self {
            authority: Authority::of(bound)?,
            shared,
        })
    }

    /// The router that answers on the door's listener: the page and its two
    /// assets, `GET /window`, `POST /submit`, and a fixed `404` for everything
    /// else, all behind the fixed `Host`.
    pub(crate) fn router(self) -> Router {
        let door = Arc::new(self);
        Router::new()
            .route("/", get(page::index))
            .route("/unlock.js", get(page::script))
            .route("/unlock.css", get(page::style))
            .route("/window", get(describe::describe))
            .route("/submit", post(submit::submit))
            .fallback(page::not_found)
            .layer(from_fn_with_state(Arc::clone(&door), fences::fixed_host))
            .layer(from_fn(fences::answer_headers))
            .with_state(door)
    }
}
