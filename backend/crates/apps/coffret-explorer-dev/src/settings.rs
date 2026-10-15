use std::net::SocketAddr;
use std::path::PathBuf;

/// What the development host stands in front of.
pub struct Settings {
    /// The loopback address the server is bound to.
    pub server: SocketAddr,
    /// The file that server publishes its key into.
    pub key_file: PathBuf,
    /// The explorer's build output.
    pub dist: PathBuf,
}
