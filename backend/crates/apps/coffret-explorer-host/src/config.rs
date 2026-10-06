use std::net::SocketAddr;
use std::path::PathBuf;

use coffret_device::LibraryDir;

/// Where the host forwards `/api`, and where it reads the key to forward with.
#[derive(Debug, Clone)]
pub struct Config {
    /// The loopback address the server is bound to.
    pub server: SocketAddr,
    /// The file that server published its key into.
    pub key_file: PathBuf,
}

impl Config {
    /// The host for the server serving `library` at `server`, reading the key
    /// from where that server publishes it.
    pub fn for_library(server: SocketAddr, library: &LibraryDir) -> Self {
        Self {
            server,
            key_file: library.server_key_file(),
        }
    }
}
