//! Keeping this device's Master Key under a new Passphrase.

use crate::library_args::WhichLibrary;
use clap::Args;

use crate::answer::{Answer, ChangedPassphrase, Ran};
use coffret_shell::passphrase;

/// What `change-passphrase` is told.
///
/// Not [`LibraryArgs`](crate::library_args::LibraryArgs), whose
/// `--passphrase-stdin` reads one line: this command reads two, and the flag
/// says which is which.
#[derive(Args)]
pub struct ChangePassphraseArgs {
    // The Library on this device whose Passphrase to change, by its name or by its ID.
    #[command(flatten)]
    which: WhichLibrary,
    /// Read the current Passphrase from the first line of standard input and
    /// the new one from the next, instead of asking for them, which is what a
    /// script does
    #[arg(long)]
    passphrase_stdin: bool,
}

pub fn run(args: ChangePassphraseArgs) -> anyhow::Result<Ran> {
    let library = args.which.library();
    coffret_device::change_passphrase(
        &library,
        passphrase::entering(args.passphrase_stdin),
        passphrase::choosing(args.passphrase_stdin),
    )?;
    let library = library.to_string();
    eprintln!("{}", changed(&library));
    Ok(Ran::clean(Answer::ChangedPassphrase(
        ChangedPassphrase::new(library),
    )))
}

/// What a person reads once the Passphrase is changed.
///
/// The one thing a person could get wrong next is to expect the new Passphrase
/// on another device: each device keeps its own, and only this one's changed
/// (spec: DK-6).
fn changed(library: &str) -> String {
    format!(
        "The Passphrase of {library} is changed on this device only.\n\
         Other devices holding {library} keep the Passphrase they have."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pinned because it is the whole of what a person is told, and the part
    // that matters is that the change stops at this device.
    #[test]
    fn a_change_says_it_reached_this_device_only() {
        assert_eq!(
            changed("photos"),
            "The Passphrase of photos is changed on this device only.\n\
             Other devices holding photos keep the Passphrase they have.",
        );
    }
}
