use std::net::SocketAddr;
use std::path::PathBuf;

use coffret_device::AppearanceFile;

/// What the development host stands in front of.
pub struct Settings {
    /// The loopback address the server is bound to.
    pub server: SocketAddr,
    /// The file that server publishes its key into.
    pub key_file: PathBuf,
    /// The explorer's build output.
    pub dist: PathBuf,
    /// Where the set of colours the explorer is drawn in is kept, which the
    /// page is served in: the file of the state directory the server beside
    /// it keeps the choice in.
    pub appearance: AppearanceFile,
}
