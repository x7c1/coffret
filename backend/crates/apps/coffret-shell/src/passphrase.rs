//! Reading a Passphrase from whoever started the run.
//!
//! Once per process, because one process is one unlock (spec: DK-9): a command
//! spends it and exits, and a server spends it at startup and holds the derived
//! keys for as long as it runs.
//!
//! Every string a terminal or a pipe produces here becomes a [`Passphrase`]
//! before anything else is done with it, so there is no intermediate to be left
//! in freed memory: the buffer the characters were read into is the buffer the
//! `Passphrase` wipes (spec: DK-7). The one place that costs a line of care is
//! trimming: the obvious `line.trim_end().as_bytes().to_vec()` leaves the
//! untrimmed original behind, so the line is truncated in place and its own
//! allocation becomes the Passphrase's.

use std::io::BufRead;

use anyhow::{bail, Context};
use coffret_device::Passphrase;

/// What `coffret-device` is handed to ask for the Passphrase of a Library that
/// already exists.
///
/// A callback rather than a value, because the device layer calls it only once
/// every refusal that needs no key has passed: a Library that is not on this
/// device, or a name that could not be one, costs nobody a prompt — and a script
/// piping a Passphrase to a command that refuses still has it unread.
///
/// What the terminal reported crosses into the device layer's vocabulary whole,
/// so a caller printing the chain still sees why the read failed.
pub fn entering(from_stdin: bool) -> impl FnOnce() -> coffret_device::Result<Passphrase> {
    move || enter(from_stdin).map_err(not_given)
}

/// The same, for the Passphrase a Library is about to be created under.
pub fn choosing(from_stdin: bool) -> impl FnOnce() -> coffret_device::Result<Passphrase> {
    move || choose(from_stdin).map_err(not_given)
}

/// What the device layer is told when the terminal produced no Passphrase.
fn not_given(cause: anyhow::Error) -> coffret_device::Error {
    coffret_device::Error::PassphraseNotGiven {
        cause: cause.into(),
    }
}

/// Reads the Passphrase of a Library that already exists.
///
/// Once, because there is a stored form to check it against: a Passphrase typed
/// wrongly is refused by the file rather than by a second prompt.
fn enter(from_stdin: bool) -> anyhow::Result<Passphrase> {
    if from_stdin {
        return read_line();
    }
    let passphrase = rpassword::prompt_password("Enter the Passphrase: ")
        .context("the Passphrase could not be read")?;
    Ok(taken(passphrase))
}

/// Reads the Passphrase a Library is about to be created under.
///
/// Twice, because there is nothing yet to check it against: this is the one
/// moment a typo would be stored rather than caught, and what it would cost is
/// the Library — losing every device that holds it and every Recovery Code
/// written down for it leaves nobody able to read it.
///
/// The one refusal both ways of giving it share is the empty one. A script that
/// pipes an empty line means the same thing a person pressing return means, and
/// the Library it would create is one the stored form protects with nothing.
fn choose(from_stdin: bool) -> anyhow::Result<Passphrase> {
    let chosen = if from_stdin {
        read_line()?
    } else {
        // Both readings become a `Passphrase` the moment they are read, so both
        // are wiped whatever happens next: the second is only ever compared
        // against the first, and the first is returned rather than copied into
        // the value that is.
        let chosen = taken(
            rpassword::prompt_password("Choose a Passphrase: ")
                .context("the Passphrase could not be read")?,
        );
        let again = taken(
            rpassword::prompt_password("Enter it again: ")
                .context("the Passphrase could not be read")?,
        );
        if chosen != again {
            bail!("the two Passphrases are not the same; nothing was created");
        }
        chosen
    };
    if chosen.is_empty() {
        bail!("an empty Passphrase protects nothing; nothing was created");
    }
    Ok(chosen)
}

/// Reads one line of standard input, without the line ending.
///
/// For a script and for a test: neither has a terminal to be prompted at, and
/// a Passphrase on the command line would sit in the shell history and in the
/// process table where anyone on the machine could read it.
fn read_line() -> anyhow::Result<Passphrase> {
    let mut line = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("the Passphrase could not be read from standard input")?;
    if read == 0 {
        bail!("standard input ended before a Passphrase was given");
    }
    Ok(taken(line))
}

/// Takes a string a terminal produced, trimmed of its line ending.
///
/// The truncation is in place and the string is then consumed, so the buffer
/// the characters were read into is the buffer the Passphrase wipes — nothing
/// is copied out of it and left behind.
fn taken(mut read: String) -> Passphrase {
    read.truncate(read.trim_end_matches(['\r', '\n']).len());
    Passphrase::from_bytes(read.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_ending_is_not_part_of_the_passphrase() {
        assert_eq!(taken("secret\r\n".to_owned()).as_bytes(), b"secret");
        assert_eq!(taken("secret\n".to_owned()).as_bytes(), b"secret");
        assert_eq!(taken("secret".to_owned()).as_bytes(), b"secret");
    }

    // Only the ending: a Passphrase a person chose with a space at the end of it
    // is that Passphrase, and trimming more than the line ending would leave
    // them unable to open what they created.
    #[test]
    fn nothing_but_the_line_ending_is_trimmed() {
        assert_eq!(taken(" secret \n".to_owned()).as_bytes(), b" secret ");
        assert_eq!(taken("\t\n".to_owned()).as_bytes(), b"\t");
    }
}
