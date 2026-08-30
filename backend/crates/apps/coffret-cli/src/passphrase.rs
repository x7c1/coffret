use std::io::BufRead;

use anyhow::{bail, Context};

/// Reads the Passphrase of a Library that already exists.
///
/// Once, because there is a stored form to check it against: a Passphrase typed
/// wrongly is refused by the file rather than by a second prompt.
pub fn enter(from_stdin: bool) -> anyhow::Result<Vec<u8>> {
    if from_stdin {
        return read_line();
    }
    let passphrase = rpassword::prompt_password("Enter the Passphrase: ")
        .context("the Passphrase could not be read")?;
    Ok(passphrase.into_bytes())
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
pub fn choose(from_stdin: bool) -> anyhow::Result<Vec<u8>> {
    let chosen = if from_stdin {
        read_line()?
    } else {
        let chosen = rpassword::prompt_password("Choose a Passphrase: ")
            .context("the Passphrase could not be read")?;
        let again = rpassword::prompt_password("Enter it again: ")
            .context("the Passphrase could not be read")?;
        if chosen != again {
            bail!("the two Passphrases are not the same; nothing was created");
        }
        chosen.into_bytes()
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
fn read_line() -> anyhow::Result<Vec<u8>> {
    let mut line = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("the Passphrase could not be read from standard input")?;
    if read == 0 {
        bail!("standard input ended before a Passphrase was given");
    }
    Ok(line.trim_end_matches(['\r', '\n']).as_bytes().to_vec())
}
