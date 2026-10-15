use std::sync::{Arc, OnceLock};

use coffret_explorer_host::launch_file::LaunchDir;
use coffret_server::ServerState;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

use crate::launch::Launcher;

/// What the app holds for the whole of the process.
pub struct Desktop {
    /// The runtime the server and the explorer's host run on.
    pub(super) runtime: Runtime,
    /// How long the Library stays open while nobody wants it, read once as
    /// the app starts.
    pub(super) idle_minutes: OnceLock<u64>,
    /// The owner-only directory launch files are written into, prepared once
    /// as the app starts (spec: LA-21).
    pub(super) launch_dir: OnceLock<LaunchDir>,
    /// Taken for as long as one attempt at opening or unlocking a Library is
    /// under way, so that a second press of the button waits for the first
    /// rather than racing it for the Library's lock or for its keys.
    pub(super) opening: Mutex<()>,
    /// What opens the explorer, once a Library is open. Set once: this app
    /// serves one Library for as long as it runs, so a generation of the host
    /// ends by the server stopping or the app quitting, never by the Library
    /// served changing (spec: LA-18).
    pub(super) explorer: OnceLock<Arc<Launcher>>,
    /// What the server serves, once a Library is open: what an unlock in place
    /// hands the reopened Library to, and what says whether one is needed.
    pub(super) served: OnceLock<Arc<ServerState>>,
}

impl Desktop {
    /// An app with nothing open yet, running its servers on `runtime`.
    pub fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            idle_minutes: OnceLock::new(),
            launch_dir: OnceLock::new(),
            opening: Mutex::new(()),
            explorer: OnceLock::new(),
            served: OnceLock::new(),
        }
    }

    /// Records the idle interval the app started with.
    pub fn set_idle_minutes(&self, minutes: u64) {
        // Set once, by the one setup hook; a second call would be a second
        // setup, which Tauri does not run.
        let _ = self.idle_minutes.set(minutes);
    }

    /// Records the directory launch files are written into.
    pub fn set_launch_dir(&self, dir: LaunchDir) {
        // Set once, by the one setup hook, as `set_idle_minutes` is.
        let _ = self.launch_dir.set(dir);
    }

    /// What opens the explorer, if a Library is open.
    pub fn explorer(&self) -> Option<&Arc<Launcher>> {
        self.explorer.get()
    }

    /// What the server serves, if a Library is open.
    pub fn served(&self) -> Option<&Arc<ServerState>> {
        self.served.get()
    }
}
