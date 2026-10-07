use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use coffret_server::ServerState;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

/// What the shell holds for the whole of the process.
pub struct Shell {
    /// The runtime the server and the explorer's host run on.
    pub(super) runtime: Runtime,
    /// How long the Library stays open while nobody wants it, read once as
    /// the shell starts.
    pub(super) idle_minutes: OnceLock<u64>,
    /// Taken for as long as one attempt at opening or unlocking a Library is
    /// under way, so that a second press of the button waits for the first
    /// rather than racing it for the Library's lock or for its keys.
    pub(super) opening: Mutex<()>,
    /// Where the explorer is, once a Library is open. Set once: this shell
    /// serves one Library for as long as it runs.
    pub(super) explorer: OnceLock<SocketAddr>,
    /// What the server serves, once a Library is open: what an unlock in place
    /// hands the reopened Library to, and what says whether one is needed.
    pub(super) served: OnceLock<Arc<ServerState>>,
}

impl Shell {
    /// A shell with nothing open yet, running its servers on `runtime`.
    pub fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            idle_minutes: OnceLock::new(),
            opening: Mutex::new(()),
            explorer: OnceLock::new(),
            served: OnceLock::new(),
        }
    }

    /// Records the idle interval the shell started with.
    pub fn set_idle_minutes(&self, minutes: u64) {
        // Set once, by the one setup hook; a second call would be a second
        // setup, which Tauri does not run.
        let _ = self.idle_minutes.set(minutes);
    }

    /// Where the explorer is, if a Library is open.
    pub fn explorer(&self) -> Option<SocketAddr> {
        self.explorer.get().copied()
    }

    /// What the server serves, if a Library is open.
    pub fn served(&self) -> Option<&Arc<ServerState>> {
        self.served.get()
    }
}
