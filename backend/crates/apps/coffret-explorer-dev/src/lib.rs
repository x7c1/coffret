//! The explorer's host for development: the explorer's build output on disk,
//! served in front of a `coffret-server` that is already running.
//!
//! The one browser entrance in every mode is the explorer host
//! (`coffret-explorer-host`), so development goes through it too rather than
//! through a dev server of the frontend's own: the same fences, the same
//! browser tokens and the same launch tickets as the desktop app, with the
//! page read from `frontend/packages/apps/web/dist` at each request instead of
//! compiled in. A frontend build in watch mode rewrites that directory, and
//! reloading the tab picks up what it wrote.
//!
//! What differs from the desktop app is only what a separate process cannot
//! do. The server is another process, so its key is read from the file it
//! publishes, as any process of the owning account can (spec: LA-3), and
//! whether its Library is locked is not something this host can see: it
//! forwards as though the Library were unlocked, and the server's own custody
//! of the keys refuses what a locked Library cannot answer ([`InAnotherProcess`]).
//! And there is no tray to open the explorer from, so the launch file is kept
//! ready instead ([`LaunchKeeper`]): written as the server is first seen and
//! printed, written again each time its ticket is spent, and written afresh —
//! the host's generation retired — whenever the server's key changes, which
//! is a server of another run behind the port (spec: LA-4, LA-18, LA-21).
//!
//! No HTTP request, environment variable or command-line argument carries a
//! ticket: a person, or a test, opens the file.

mod in_another_process;
pub use in_another_process::InAnotherProcess;

mod launch_keeper;
pub use launch_keeper::{LaunchKeeper, REFRESH_AFTER};

mod settings;
pub use settings::Settings;

mod tick;
pub use tick::Tick;

use std::sync::Arc;

use axum::Router;
use coffret_explorer_host::credentials::{Credentials, SystemClock};
use coffret_explorer_host::{router, Config, HostKind, KeyFile, PageSource};
use tokio::net::TcpListener;

/// The name the launch file is written under, in its directory: the same
/// every time, so that the path printed once stays the one to open.
pub const LAUNCH_FILE: &str = "explorer.html";

/// The router for a development host on `listener`, and the credentials it
/// admits browsers by.
pub fn host(listener: &TcpListener, settings: &Settings) -> anyhow::Result<(Router, Credentials)> {
    let credentials = Credentials::new(Arc::new(SystemClock))?;
    let config = Config {
        server: settings.server,
        key: Arc::new(KeyFile(settings.key_file.clone())),
        lock: Arc::new(InAnotherProcess),
        page: PageSource::Directory(settings.dist.clone()),
        kind: HostKind::Development,
    };
    let router = router(listener, config, credentials.gate())?;
    Ok((router, credentials))
}
