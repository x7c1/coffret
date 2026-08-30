//! The arguments more than one subcommand takes, which is why they are here
//! rather than in either of them.

use clap::Args;

/// What a command acting on one Library this device already has is told.
#[derive(Args)]
pub struct LibraryArgs {
    /// The Library on this device to act on
    #[arg(long)]
    pub library: String,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it, which is what a script does
    #[arg(long)]
    pub passphrase_stdin: bool,
}
