//! Keeping this device's Master Key under a new Passphrase.

use crate::library_args::WhichLibrary;
use clap::Args;

use crate::answer::{Answer, ChangedPassphrase, Ran};
use coffret_device::LibraryOnDevice;
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
    let library = coffret_device::change_passphrase(
        &args.which.library(),
        passphrase::entering(args.passphrase_stdin),
        passphrase::choosing(args.passphrase_stdin),
    )?;
    eprintln!("{}", changed(&library));
    Ok(Ran::clean(Answer::ChangedPassphrase(
        ChangedPassphrase::new(library),
    )))
}

/// What a person reads once the Passphrase is changed.
///
/// The one thing a person could get wrong next is to expect the new Passphrase
/// on another device: each device keeps its own, and only this one's changed
/// (spec: DK-6). The Library is named by its name, or — where this device
/// knows none yet — said to have none and named by its ID.
fn changed(library: &LibraryOnDevice) -> String {
    format!(
        "The Passphrase of {} is changed on this device only.\n\
         Other devices holding it keep the Passphrase they have.",
        library.which().the_library()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pinned because it is the whole of what a person is told, and the part
    // that matters is that the change stops at this device.
    #[test]
    fn a_change_says_it_reached_this_device_only() {
        let id = coffret_device::LibraryId::from_bytes([0xab; coffret_device::LibraryId::BYTE_LEN]);
        assert_eq!(
            changed(&LibraryOnDevice {
                library_id: id,
                name: Some("photos".to_owned()),
                needs_attention: false,
            }),
            "The Passphrase of the Library \"photos\" is changed on this device only.\n\
             Other devices holding it keep the Passphrase they have.",
        );
    }

    // A Library with no name yet is said to have none, and named by its ID
    // as an ID rather than in the name's place.
    #[test]
    fn a_change_to_a_library_with_no_name_names_it_by_its_id() {
        let id = coffret_device::LibraryId::from_bytes([0xab; coffret_device::LibraryId::BYTE_LEN]);
        assert_eq!(
            changed(&LibraryOnDevice {
                library_id: id,
                name: None,
                needs_attention: false,
            }),
            "The Passphrase of the Library with ID abababababababab (it has no name yet) is \
             changed on this device only.\n\
             Other devices holding it keep the Passphrase they have.",
        );
    }
}
