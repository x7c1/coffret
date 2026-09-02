//! Serving one Library on this device to a browser on it.
//!
//! Everything but starting up is in the library half of this crate; its
//! documentation says what the browser is told and what it is not. This is the
//! order a process starts in: the log first, then the Passphrase, then the
//! Library, then the catalog caught up with what the Library has become, then
//! the key this run admits its callers by, then a socket, and beside it the
//! task that locks the Library again once nobody has wanted it for the idle
//! interval (spec: DK-4). Every step but the catch-up is fatal where it fails.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use coffret_device::{open_library, LibraryDir, ServerKey};
use coffret_server::{
    catch_up_at_startup, lock_when_idle, router, Admission, ServerState, CAPABILITY_HEADER,
};

#[derive(Parser)]
#[command(
    name = "coffret-server",
    about = "Serve a Library on this device to the explorer in your browser"
)]
struct Args {
    /// The Library on this device to serve
    #[arg(long)]
    library: String,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it, which is what a script does
    #[arg(long)]
    passphrase_stdin: bool,
    /// The loopback port to listen on
    #[arg(long, default_value_t = 8787)]
    port: u16,
    /// Lock this server's hold on the Library after this many minutes in which
    /// nothing is read from or written to it, after which the Passphrase is
    /// needed again
    ///
    /// What the interval measures is somebody wanting the Library, not a
    /// browser being pointed at this port: an explorer left open asks what the
    /// server is doing several times a second, and none of that asking counts.
    ///
    /// A policy parameter and not a constant of the format (spec: DK-4): how
    /// long a device stays unlocked while nobody is using it is the owner's
    /// choice, and this is where they make it. The default is long enough that
    /// somebody reading a book is not shut out between pages, and short enough
    /// that a machine left alone for an afternoon is not holding a Master Key
    /// when they come back to it.
    #[arg(
        long,
        env = "COFFRET_IDLE_MINUTES",
        default_value_t = 30,
        value_parser = clap::value_parser!(u64).range(1..),
    )]
    idle_minutes: u64,
}

#[tokio::main]
async fn main() -> ExitCode {
    // Parsed here rather than through `parse`, so that what a person typed
    // wrongly exits the way everything else that failed does.
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let _ = error.print();
            // `--help` and `--version` arrive here too, and they are answers
            // rather than refusals.
            return match error.use_stderr() {
                true => ExitCode::FAILURE,
                false => ExitCode::SUCCESS,
            };
        }
    };

    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // The whole chain: what failed, and under it what each layer
            // reported. Nothing has been bound by the time any of these is
            // reported.
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Opens the Library, then serves it until the process is stopped.
async fn run(args: Args) -> anyhow::Result<()> {
    coffret_shell::logging::start()?;

    // Before the socket, deliberately. Every refusal opening a Library owes — it
    // is not on this device, the Passphrase does not open it, the grant has run
    // out — is one a person acts on, and a server that had already bound a port
    // would state it once per request instead of once.
    let library = open_library(
        &args.library,
        coffret_shell::passphrase::entering(args.passphrase_stdin),
    )
    .await?;
    let state = Arc::new(ServerState::new(args.library.clone(), library));

    // Before the socket as well, and for a different reason than the unlock
    // above: not because the refusal has to come before anything is bound, but
    // because the first window this server answers ought to show the Library
    // rather than whatever this device knew when it last looked (spec: CK-9) — a
    // device that has just joined knowing nothing at all. It is on a deadline of
    // its own, so a Storage that answers neither yes nor no delays the socket
    // rather than withholding it.
    catch_up_at_startup(&state).await;

    // Before the socket for the reason the unlock is: a key that could not be
    // drawn or could not be written is a server nothing legitimate could ask
    // anything of, and one that had already bound a port would say so once per
    // request instead of once. It replaces whatever a previous run left, so the
    // file a caller reads is always this server's.
    let key = ServerKey::publish(&LibraryDir::resolve(&args.library)?)?;

    // Loopback and nothing else: these routes carry the Library's plaintext, and
    // an interface anybody else is on would be that plaintext offered to whoever
    // else is on the network. Who is answered *on* this device is the key's
    // business rather than the address's. See the crate documentation.
    let address = format!("127.0.0.1:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("nothing could listen at {address}"))?;

    // What was bound rather than what was asked for. `--port 0` is how a script
    // asks the operating system for a free one, and the number it chose is then
    // the only one anything can reach the Library at — a line naming the port
    // that was typed would be naming the one place the server is not.
    let bound = listener
        .local_addr()
        .context("the address the server is listening at could not be read")?;
    eprintln!(
        "Serving the Library {:?} at http://{bound}.",
        state.name.as_str()
    );
    // The file and never what is in it. Whoever started this server is the one
    // person entitled to read it, and a terminal is somewhere a key would be
    // scrolled back through, copied into a bug report, and captured by whatever
    // is recording the session.
    //
    // The header is named beside the path because the path on its own is half a
    // recipe. The explorer never needs either — the proxy in front of it reads
    // the file and puts the header on what it forwards — but a script on this
    // device has nowhere else to learn where to put what it read, and a refusal
    // deliberately will not tell it.
    eprintln!(
        "Callers are admitted by the key at {}, sent as {CAPABILITY_HEADER}.",
        key.path().display()
    );

    // What the Library is open until, if nobody says otherwise first
    // (spec: DK-4). Said on the way up, beside where the server is and how it
    // admits callers, because it is the third thing about this run somebody has
    // to know: a Library that has locked itself refuses everything until the
    // Passphrase opens it again, and a person who was never told the interval
    // would read that as the server having broken.
    // Saturating, because the parser's only bound is that it is at least one:
    // a number large enough to wrap the multiplication would otherwise become a
    // tiny interval, and a server that locked itself at once because somebody
    // asked for a million years is the opposite of what they asked for.
    let idle = Duration::from_secs(args.idle_minutes.saturating_mul(60));
    eprintln!(
        "It locks itself after {} minute(s) in which nothing is read from or \
         written to the Library; start it again with the Passphrase to unlock \
         it.",
        args.idle_minutes,
    );

    let admission = Arc::new(Admission::new(bound.to_string(), key.secret()));

    // Started before the socket is served rather than after, so that a server
    // nobody ever asks anything of still locks.
    tokio::spawn(lock_when_idle(Arc::clone(&state), idle));

    axum::serve(listener, router(state, admission))
        .await
        .context("the server stopped")
}
