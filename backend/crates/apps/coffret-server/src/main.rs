//! Serving one Library on this device to a browser on it.
//!
//! Everything but reading the command line is in the library half of this
//! crate; its documentation says what the browser is told and what it is not,
//! and [`Launch`] says the order a server starts in. This binary starts the log
//! first, asks for the Passphrase on the terminal (or reads it from standard
//! input), and says on the terminal where the server is once it is bound.

use std::process::ExitCode;

use clap::Parser;
use coffret_server::{Launch, SERVER_KEY_HEADER};

#[derive(Parser)]
#[command(
    name = "coffret-server",
    version,
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
    // Before clap, because clap refuses an argument it did not expect by
    // quoting it — and a value typed after `--passphrase-stdin` is the
    // Passphrase itself (spec: DK-10). What is said instead is what the flag
    // is. A secret typed with no flag in front of it has nothing to recognise
    // before parsing, and is caught below instead.
    if let Some(refusal) =
        coffret_shell::stdin_flags::value_typed_after_a_secret_flag(std::env::args_os())
    {
        eprintln!("error: {refusal}");
        return ExitCode::FAILURE;
    }

    // Parsed here rather than through `parse`, so that what a person typed
    // wrongly exits the way everything else that failed does.
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            // The other half of the guard above: an argument clap did not
            // expect is one it would refuse by quoting, and this binary takes
            // no bare argument at all, so one typed here is where a Passphrase
            // pasted by hand lands (spec: DK-10). The refusal says so without
            // repeating it.
            if let Some(refusal) =
                coffret_shell::parser_refusal::argument_refused_without_being_quoted(&error)
            {
                eprintln!("error: {refusal}");
                return ExitCode::FAILURE;
            }
            // Everything else is clap's own to print, a mistyped flag included:
            // a flag is not a secret, and which one was wrong is what the
            // person needs to see.
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

    let launch = Launch {
        library: args.library,
        port: args.port,
        idle_minutes: args.idle_minutes,
        // The terminal is read once, as the server starts, and nothing reads it
        // afterwards: a server of this binary is unlocked by being started
        // again, and its locked refusal says so.
        unlock_prompt: None,
    };
    let idle_minutes = launch.idle_minutes;
    let serving = launch
        .open(coffret_shell::passphrase::entering(args.passphrase_stdin))
        .await?;

    eprintln!(
        "Serving the Library {:?} at http://{}.",
        serving.state().name.as_str(),
        serving.address(),
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
        "Callers are admitted by the key at {}, sent as {SERVER_KEY_HEADER}.",
        serving.key_file().display()
    );

    // What the Library is open until, if the server is not stopped first
    // (spec: DK-4). Said on the way up, beside where the server is and how it
    // admits callers, because it is the third thing about this run somebody has
    // to know: a Library that has locked itself refuses everything until the
    // Passphrase opens it again, and a person who was never told the interval
    // would read that as the server having broken.
    eprintln!(
        "It locks itself after {idle_minutes} minute(s) in which nothing is read from or \
         written to the Library; a server started from the command line takes the \
         Passphrase only as it starts, so start it again with the Passphrase to unlock it.",
    );

    serving.serve().await
}
