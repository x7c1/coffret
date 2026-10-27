//! The explorer's host for development, in front of a `coffret-server` that is
//! already running: what `make web` and `make dev` serve the explorer through.
//!
//! ```text
//! coffret-explorer-dev --library main --server-port 8787 --port 5173
//! ```
//!
//! `--library` names the Library by its name and `--library-id` by its ID, as
//! every binary takes them. The Library's directory is found the way the
//! binaries find it, so `COFFRET_STATE_DIR` has to name the state directory
//! that server runs from; the Makefile sets it for its own targets. The page is
//! read from the explorer's build output on disk,
//! `frontend/packages/apps/web/dist` unless `--dist` names another, so a build
//! in watch mode beside this is picked up by reloading the tab.
//!
//! A browser is admitted only by a browser token, which a page gets by
//! exchanging the ticket in the launch file this prints the path of. Open that
//! file in the browser; the crate's documentation says when it is written
//! again.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context;
use clap::Parser;
use coffret_device::{LibraryDir, LibraryId};
use coffret_explorer_dev::{host, LaunchKeeper, Settings, Tick};
use coffret_explorer_host::launch_file::LaunchDir;
use coffret_explorer_host::{serve, KeyFile};

/// How often the server's key and the launch file are looked at.
const LOOK_EVERY: Duration = Duration::from_millis(250);

/// The explorer's build output, as this checkout lays it out.
const DIST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../frontend/packages/apps/web/dist"
);

#[derive(Parser)]
#[command(
    name = "coffret-explorer-dev",
    version,
    about = "Serve the explorer's build on disk in front of a running coffret server"
)]
#[command(group(clap::ArgGroup::new("which_library").required(true).args(["library", "library_id"])))]
struct Args {
    /// The Library the server is serving, by its name. Where more than one
    /// Library on this device carries the name, the start is refused with
    /// their IDs, and --library-id says which one. Where more than one
    /// directory holds the Library, it is refused with both directories
    #[arg(long)]
    library: Option<String>,
    /// The Library the server is serving, by its ID: the 16 lowercase hex
    /// characters `coffret init` and `coffret join` print, and how a Library
    /// with no name yet is named. Where more than one directory holds the
    /// Library, it is refused with both directories, as it is by --library
    #[arg(long, value_parser = coffret_shell::which_library::library_id)]
    library_id: Option<LibraryId>,
    /// The loopback port the server is listening on
    #[arg(long, default_value_t = 8787)]
    server_port: u16,
    /// The loopback port to serve the explorer on
    #[arg(long, default_value_t = 5173)]
    port: u16,
    /// The explorer's build output
    #[arg(long, default_value = DIST)]
    dist: PathBuf,
    /// The owner-only directory the launch file is written into; by default
    /// `explorer-launch` in the Library's own directory
    #[arg(long)]
    launch_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Args::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> anyhow::Result<()> {
    coffret_shell::logging::start()?;

    let which = coffret_shell::which_library::library(args.library, args.library_id);
    let library = LibraryDir::find(&which).context("finding the Library's directory")?;
    let launch_dir = args
        .launch_dir
        .unwrap_or_else(|| library.path().join("explorer-launch"));
    let launch_dir = LaunchDir::prepare(&launch_dir)
        .with_context(|| format!("preparing {} for launch files", launch_dir.display()))?;

    let server = SocketAddr::from((Ipv4Addr::LOCALHOST, args.server_port));
    let settings = Settings {
        server,
        key_file: library.server_key_file(),
        dist: args.dist,
    };
    // Bound first, so that the host is built around the address it is on.
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, args.port))
        .await
        .with_context(|| format!("binding 127.0.0.1:{}", args.port))?;
    let address = listener.local_addr()?;
    let (router, credentials) = host(&listener, &settings)?;

    println!(
        "The explorer is served at http://{address}/ from {}, in front of the server at \
         http://{server}, with the key at {}.",
        settings.dist.display(),
        settings.key_file.display(),
    );
    println!(
        "A browser is let in by opening the launch file, which is written at {} once the \
         server is up.",
        launch_dir
            .path()
            .join(coffret_explorer_dev::LAUNCH_FILE)
            .display(),
    );

    let keeper = LaunchKeeper::new(
        credentials,
        launch_dir,
        address,
        server,
        Arc::new(KeyFile(settings.key_file.clone())),
    );
    let stopping = Arc::new(AtomicBool::new(false));
    let keeping = {
        let stopping = Arc::clone(&stopping);
        thread::spawn(move || keep(keeper, &stopping))
    };

    tokio::select! {
        () = serve(listener, router) => {}
        stopped = tokio::signal::ctrl_c() => stopped.context("waiting for Ctrl-C")?,
        () = terminated() => {}
    }
    stopping.store(true, Ordering::Relaxed);
    keeping
        .join()
        .map_err(|_| anyhow::anyhow!("the launch file's keeper panicked"))?;
    Ok(())
}

/// Looks at the server and the launch file until `stopping`, saying what a
/// person has to do about what it finds, then removes the file.
fn keep(mut keeper: LaunchKeeper, stopping: &AtomicBool) {
    // Said once for as long as it stands, rather than at every look.
    let mut failing: Option<String> = None;
    while !stopping.load(Ordering::Relaxed) {
        let looked = keeper.tick(Instant::now());
        if looked.is_ok() {
            failing = None;
        }
        match looked {
            Ok(Tick::Opened(path)) => println!(
                "The server is up. Open the explorer by opening {} in the browser.",
                path.display()
            ),
            Ok(Tick::ServerChanged(path)) => println!(
                "The server was started again, so every tab's session ended. Open {} in the \
                 browser again.",
                path.display()
            ),
            Ok(Tick::Withdrawn) => println!(
                "The server is not answering, so the launch file was taken away; another is \
                 written once it answers again."
            ),
            Ok(Tick::Rearmed | Tick::Quiet) => {}
            Err(error) => {
                // With its causes, as `main` says a failure: the sentence alone
                // names the step and drops what the file system said.
                let said = format!("{:#}", anyhow::Error::new(error));
                if failing.as_deref() != Some(said.as_str()) {
                    eprintln!("{said}");
                    failing = Some(said);
                }
            }
        }
        thread::sleep(LOOK_EVERY);
    }
    if let Err(cause) = keeper.close() {
        eprintln!("The launch file could not be removed: {cause}");
    }
}

/// Resolves when the process is asked to stop, as `make down` asks it.
async fn terminated() {
    let Ok(mut signal) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    else {
        return std::future::pending().await;
    };
    signal.recv().await;
}
