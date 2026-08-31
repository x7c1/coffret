//! The command line a person keeps a Library from.
//!
//! Everything here is a shell. Each subcommand reads what was typed, hands
//! `coffret-device` a way to ask for the Passphrase where one is needed, calls
//! it, and prints what came back; no flow, no layout, and no decision about
//! where a Library lives is made in this crate. That is what lets the
//! browser-based explorer's server do the same things without either of them
//! being the odd one out — and it is why the manifest depends on
//! `coffret-device` and on nothing beneath it.
//!
//! Starting a process is the one part not written here either: pointing the run
//! at its log file and reading a Passphrase from the terminal are the same in
//! both binaries, so both take them from `coffret-shell`.
//!
//! What a run did goes to a log file under the state directory, and the file it
//! chose is printed to standard error so that whoever started the run can find
//! it. Standard output carries only what was asked for, so a Recovery Code, a
//! list of mappings, or a run's summary and findings can be piped somewhere.
//!
//! # Three exit statuses
//!
//! `0` is a run that did everything it was asked to. `1` is a run that failed.
//! `2` is the one in between, and it is why the statuses are worth spelling out:
//! a sync that returns successfully may still have left a changed file inside a
//! Pack, and a fetch may still have declined half a folder (spec: PK-14,
//! EP-11). Those are findings rather than failures — the run did what it could
//! and said what it did not do — and a script that only asked whether the
//! command failed would call them a backup. So they get a status of their own.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod authorize;
mod consent;
mod drive_client;
mod fetch;
mod freeze;
mod init;
mod join;

mod library_args;
use library_args::LibraryArgs;

mod map;
mod mappings;
mod recovery_code;

mod report;
use report::Report;

mod storage_location;
mod sync;

/// What a run that succeeded and left findings exits with.
const FINDINGS: u8 = 2;

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
    /// Take up a Library another device created, from its Recovery Code
    Join(join::JoinArgs),
    /// Renew this device's grant on a Library's Storage provider
    Authorize(LibraryArgs),
    /// Record that a folder on this device holds part of the Library
    Map(map::MapArgs),
    /// List what this device has mapped
    Mappings(mappings::MappingsArgs),
    /// Print the Library's Recovery Code again
    RecoveryCode(LibraryArgs),
    /// Carry the mapped folders into the Library
    Sync(LibraryArgs),
    /// Pack the eligible local files in a folder directly into Packs
    Freeze(freeze::FreezeArgs),
    /// Put the Library into the mapped folders
    Fetch(fetch::FetchArgs),
}

#[tokio::main]
async fn main() -> ExitCode {
    // Parsed here rather than through `parse`, so that what a person typed
    // wrongly exits the way everything else that failed does: clap's own status
    // for a usage error is the one this binary spends on findings.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
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

    match run(cli).await {
        Ok(Report::Clean) => ExitCode::SUCCESS,
        Ok(Report::Findings) => ExitCode::from(FINDINGS),
        Err(error) => {
            // The whole chain: what failed, and under it what each layer
            // reported, down to the format crate's or the provider's own words.
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Everything but deciding what to exit with.
async fn run(cli: Cli) -> anyhow::Result<Report> {
    coffret_shell::logging::start()?;

    match cli.command {
        Command::Init(args) => init::run(args).await,
        Command::Join(args) => join::run(args).await,
        Command::Authorize(args) => authorize::run(args).await,
        Command::Map(args) => map::run(args).await,
        Command::Mappings(args) => mappings::run(args).await,
        Command::RecoveryCode(args) => recovery_code::run(args),
        Command::Sync(args) => sync::run(args).await,
        Command::Freeze(args) => freeze::run(args).await,
        Command::Fetch(args) => fetch::run(args).await,
    }
}
