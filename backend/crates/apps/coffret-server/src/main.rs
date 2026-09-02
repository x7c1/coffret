//! Serving one Library on this device to a browser on it.
//!
//! Everything but starting up is in the library half of this crate; its
//! documentation says what the browser is told and what it is not. This is the
//! order a process starts in: the log first, then the Passphrase, then the
//! Library, then the catalog caught up with what the Library has become, then
//! the key this run admits its callers by, and only then a socket. Every step
//! but the catch-up is fatal where it fails.

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use coffret_device::{open_library, LibraryDir, ServerKey};
use coffret_server::{catch_up_at_startup, router, Admission, ServerState, CAPABILITY_HEADER};

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

    let admission = Arc::new(Admission::new(bound.to_string(), key.secret()));

    axum::serve(listener, router(state, admission))
        .await
        .context("the server stopped")
}
