//! Command-line driver for the cross-implementation fixture exchange.
//!
//! `make interop` runs the three steps in order: this binary generates a
//! fixture set, the TypeScript suite reads it and writes one back, and this
//! binary verifies that one. A non-zero exit from either step fails the
//! exchange.

use std::path::PathBuf;

use anyhow::Result;
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
}

fn main() -> Result<()> {
    match Args::parse().command {
        Command::Generate { out } => coffret_interop::generate(&out),
        Command::Verify { input } => coffret_interop::verify(&input),
    }
}
