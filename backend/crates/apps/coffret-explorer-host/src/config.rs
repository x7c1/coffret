use std::net::SocketAddr;
use std::sync::Arc;

use coffret_device::LibraryDir;
use coffret_server::ServerState;

use crate::{KeyFile, KeySource, LibraryLock, PageSource};

/// What the host stands in front of: where it forwards `/api`, where it reads
/// the key to forward with, how it learns whether the Library is locked, and
/// where the page it serves comes from.
#[derive(Clone)]
pub struct Config {
    /// The loopback address the server is bound to.
    pub server: SocketAddr,
    /// Where the key that server published is read from.
    pub key: Arc<dyn KeySource>,
    /// Whether the Library that server serves is locked.
    pub lock: Arc<dyn LibraryLock>,
    /// Where the explorer's page is served from.
    pub page: PageSource,
}

impl Config {
    /// The host for the server serving `library` at `server`, reading the key
    /// from where that server publishes it and the lock from what it serves,
    /// and serving the page compiled into the binary.
    pub fn for_library(server: SocketAddr, library: &LibraryDir, state: Arc<ServerState>) -> Self {
        Self {
            server,
            key: Arc::new(KeyFile(library.server_key_file())),
            lock: state,
            page: PageSource::Embedded,
        }
    }
}
