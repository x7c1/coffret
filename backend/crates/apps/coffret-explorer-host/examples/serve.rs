//! The explorer's host on a port of its own, in front of a server that is
//! already running: what the desktop shell will do in-process, runnable today
//! against `make dev`'s server.
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

use std::net::{Ipv4Addr, SocketAddr};

use anyhow::Context;
use clap::Parser;
use coffret_device::LibraryDir;
use coffret_explorer_host::{router, Config};

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
    let config = Config::for_library(args.server, &library);
    println!(
        "/api goes to http://{}, with the key at {}",
        config.server,
        config.key_file.display()
    );
    let host = router(config)?;

    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, args.port))
        .await
        .context("binding the explorer's port")?;
    let address = listener.local_addr()?;
    println!("The explorer is at http://{address}/");
    axum::serve(listener, host).await?;
    Ok(())
}
