//! The arguments more than one subcommand takes, which is why they are here
//! rather than in either of them.

use clap::Args;
use coffret_device::LibraryRef;

mod which_library;
pub use which_library::{library_id, WhichLibrary};

/// What a command acting on one Library this device already has is told.
#[derive(Args)]
pub struct LibraryArgs {
    #[command(flatten)]
    pub which: WhichLibrary,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it, which is what a script does
    #[arg(long)]
    pub passphrase_stdin: bool,
}

impl LibraryArgs {
    /// The Library the flags name.
    pub fn library(&self) -> LibraryRef {
        self.which.library()
    }
}
