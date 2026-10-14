//! The explorer's host on a port of its own, in front of a server that is
//! already running: what the desktop app does in-process, runnable against
//! `make dev`'s server.
//!
//! ```text
//! make web-dist
//! cd backend && COFFRET_STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/coffret-dev" \
//!     cargo run -p coffret-explorer-host --features embed-web --example serve -- \
//!     --server 127.0.0.1:8787 --library main
//! ```
//!
//! The Library's directory is resolved the way the binaries resolve it, so
//! `COFFRET_STATE_DIR` has to name the state directory that server runs from.
//! The Makefile sets it for its own targets — the development state directory
//! above, unless a `local.mk` names another — and a `cargo run` outside the
//! Makefile does not inherit it: left unset, it resolves to the production state
//! directory, whose key the server under `make dev` does not accept. The path
//! printed as this starts is the file it reads, to set beside the one the server
//! printed.
//!
//! The host admits a browser only by a browser token, which a page gets by
//! exchanging a launch ticket the process that owns the host issues
//! (spec: LA-17). This example issues none, so its page loads but reads no
//! Library data.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use coffret_device::LibraryDir;
use coffret_explorer_host::credentials::{Credentials, SystemClock};
use coffret_explorer_host::{router, serve, Config, KeyFile, LibraryLock};

#[derive(Parser)]
struct Args {
    /// The loopback address the coffret server is bound to.
    #[arg(long)]
    server: SocketAddr,
    /// The name of the Library that server is serving.
    #[arg(long)]
    library: String,
    /// The port to serve the explorer on; 0 takes any free one.
    #[arg(long, default_value_t = 0)]
    port: u16,
}

/// A server in another process, whose lock this one cannot see: the host
/// forwards as though it were unlocked, and the server's own custody of the
/// keys refuses what a locked Library cannot answer.
struct InAnotherProcess;

impl LibraryLock for InAnotherProcess {
    fn is_locked(&self) -> bool {
        false
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // What the host says about a request it could not forward, a key file it
    // could not read, or an answer that broke off midway goes nowhere without
    // a subscriber, and the page sees only the outcome.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let library =
        LibraryDir::resolve(&args.library).context("resolving the Library's directory")?;
    let key_file = library.server_key_file();
    println!(
        "/api goes to http://{}, with the key at {}",
        args.server,
        key_file.display()
    );
    let config = Config {
        server: args.server,
        key: Arc::new(KeyFile(key_file)),
        lock: Arc::new(InAnotherProcess),
    };

    // Bound first, so that the host is built around the address it is on.
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, args.port))
        .await
        .context("binding the explorer's port")?;
    let address = listener.local_addr()?;
    let credentials = Credentials::new(Arc::new(SystemClock))?;
    let host = router(&listener, config, credentials.gate())?;
    println!("The explorer is at http://{address}/");
    serve(listener, host).await;
    Ok(())
}
