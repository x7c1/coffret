use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use coffret_device::ServerLock;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::{lock_when_idle, router, Admission, ServerState};

/// A server with its Library open and its socket bound, not yet answering.
///
/// What the caller learns here — where the server is, and which file holds the
/// key it admits callers by — is what it needs to say before serving starts,
/// or to put a host in front of it. [`Serving::serve`] is the rest.
pub struct Serving {
    pub(super) address: SocketAddr,
    pub(super) key_file: PathBuf,
    pub(super) idle: Duration,
    pub(super) admission: Arc<Admission>,
    pub(super) state: Arc<ServerState>,
    pub(super) listener: TcpListener,
    // This server's hold on the Library (spec: LA-8), released when serving
    // ends rather than before. `None` only for a Library that was not on this
    // device, which `open_library` has refused by the time a `Serving` exists.
    pub(super) _hold: Option<ServerLock>,
}

impl Serving {
    /// The loopback address the server is bound to.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The file the key this run admits its callers by is in.
    pub fn key_file(&self) -> &Path {
        &self.key_file
    }

    /// What is being served.
    pub fn state(&self) -> &Arc<ServerState> {
        &self.state
    }

    /// Answers on the bound socket until serving fails, with the task beside it
    /// that locks the Library once nobody has wanted it for the idle interval
    /// (spec: DK-4).
    ///
    /// The lock task starts before the first request is answered rather than
    /// after, so that a server nobody ever asks anything of still locks, and it
    /// is stopped when serving ends: a Library nobody can reach any more has
    /// nobody left to lock it against.
    pub async fn serve(self) -> anyhow::Result<()> {
        let Serving {
            idle,
            admission,
            state,
            listener,
            _hold,
            ..
        } = self;
        let _locking = AbortOnDrop(tokio::spawn(lock_when_idle(Arc::clone(&state), idle)));
        axum::serve(listener, router(state, admission))
            .await
            .context("the server stopped")
    }
}

/// A task that ends when whoever started it does.
struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
