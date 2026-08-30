//! The command line a person sets a Library up from.
//!
//! Everything here is a shell. Each subcommand reads what was typed, asks for
//! the Passphrase where one is needed, calls `coffret-device`, and prints what
//! came back; no flow, no layout, and no decision about where a Library lives
//! is made in this crate. That is what lets the browser-based explorer do the
//! same things later without either of them being the odd one out — and it is
//! why the manifest depends on `coffret-device` and on nothing beneath it.
//!
//! What a run did goes to a log file under the state directory, and the file it
//! chose is printed to standard error so that whoever started the run can find
//! it. Standard output carries only what was asked for, so a Recovery Code or a
//! list of mappings can be piped somewhere.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod authorize;
mod consent;
mod init;

mod library_args;
use library_args::LibraryArgs;

mod logging;
mod map;
mod mappings;
mod passphrase;
mod recovery_code;

#[derive(Parser)]
#[command(
    name = "coffret",
    about = "Keep a folder in an encrypted Library on Storage you do not have to trust"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a Library on this device and on Storage
    Init(init::InitArgs),
    /// Renew this device's grant on a Library's Storage provider
    Authorize(LibraryArgs),
    /// Record that a folder on this device holds part of the Library
    Map(map::MapArgs),
    /// List what this device has mapped
    Mappings(mappings::MappingsArgs),
    /// Print the Library's Recovery Code again
    RecoveryCode(LibraryArgs),
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // The whole chain: what failed, and under it what each layer
            // reported, down to the format crate's or the provider's own words.
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Everything but deciding what to exit with.
async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    logging::start()?;

    match cli.command {
        Command::Init(args) => init::run(args).await,
        Command::Authorize(args) => authorize::run(args).await,
        Command::Map(args) => map::run(args).await,
        Command::Mappings(args) => mappings::run(args).await,
        Command::RecoveryCode(args) => recovery_code::run(args),
    }
}
