//! Command-line driver for the cross-implementation fixture exchange.
//!
//! `make interop` runs the three steps in order: this binary generates a
//! fixture set, the TypeScript suite reads it and writes one back, and this
//! binary verifies that one. A non-zero exit from either step fails the
//! exchange.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Exchange format fixtures with the TypeScript implementation")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Write a fixture set for the TypeScript implementation to read
    Generate {
        /// Directory to write the fixture set into
        #[arg(long)]
        out: PathBuf,
    },
    /// Open a fixture set the TypeScript implementation wrote
    Verify {
        /// Directory holding the fixture set
        #[arg(long = "in")]
        input: PathBuf,
    },
    /// Print one control object's payload, keyed by field name, or a
    /// Container's meta section
    Dump {
        /// File holding the object's bytes, as fetched from Storage
        #[arg(long)]
        object: PathBuf,
        /// The name the object is stored under, e.g. `head-00000000000000000003`
        #[arg(long)]
        name: String,
        /// File holding the Library's Recovery Code
        #[arg(long)]
        recovery_code: PathBuf,
        /// A Keyring replica holding the Container's key, saved under the name
        /// it is stored under; needed to render a Container
        #[arg(long)]
        keyring: Option<PathBuf>,
    },
}

/// Runs the step asked for, and on a failure prints the whole chain of causes
/// — the fixture, the field and the lower layer's own answer — and exits
/// non-zero.
fn main() -> ExitCode {
    match run(Args::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let mut links = error.chain().into_iter();
            if let Some(first) = links.next() {
                eprintln!("Error: {first}");
            }
            for link in links {
                eprintln!("  caused by: {link}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> coffret_interop::Result<()> {
    match command {
        Command::Generate { out } => coffret_interop::generate(&out),
        Command::Verify { input } => coffret_interop::verify(&input),
        Command::Dump {
            object,
            name,
            recovery_code,
            keyring,
        } => {
            println!(
                "{}",
                coffret_interop::dump(&object, &name, &recovery_code, keyring.as_deref())?
            );
            Ok(())
        }
    }
}
