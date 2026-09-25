//! Reading a Passphrase from whoever started the run.
//!
//! Once per process, because one process is one unlock (spec: DK-9): a command
//! spends it and exits, and a server spends it at startup and holds the derived
//! keys until a lock ends them (spec: DK-1, DK-7) — after which it is started
//! afresh, which is another process and another reading of the Passphrase. A
//! lock leaves nothing of the derived keys behind rather than storing them
//! protected: what a locked device still holds is the stored Master Key this
//! Passphrase was read to open.
//!
//! Every string a terminal or a pipe produces here becomes a [`Passphrase`]
//! before anything else is done with it, so there is no intermediate to be left
//! in freed memory: the buffer the characters were read into is the buffer the
//! `Passphrase` wipes (spec: DK-7). The one place that costs a line of care is
//! trimming: the obvious `line.trim_end().as_bytes().to_vec()` leaves the
//! untrimmed original behind, so the line is truncated in place and its own
//! allocation becomes the Passphrase's.

use std::io::BufRead;

use coffret_device::Passphrase;

use crate::error::{Error, Result, Secret, Source};

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

/// What `coffret-device` is handed to ask for the Passphrase of a Library that
/// already references the account a new Library is going onto, where the new
/// Library's own Passphrase opens none of them (spec: SA-9).
///
/// A script reading its one Passphrase from standard input has nobody to ask,
/// so it is handed [`unasked`](coffret_device::ReferencingPassphrase::unasked)
/// and the device layer refuses instead, naming the Library whose Passphrase
/// would open the account. At a terminal the prompt names that Library too:
/// it is the person's own name for it, said to the person, and it goes nowhere
/// else.
pub fn referencing(from_stdin: bool) -> coffret_device::ReferencingPassphrase {
    if from_stdin {
        return coffret_device::ReferencingPassphrase::unasked();
    }
    coffret_device::ReferencingPassphrase::asking(|library| {
        rpassword::prompt_password(referencing_prompt(library))
            .map(taken)
            .map_err(Error::unread(Secret::Passphrase, Source::Terminal))
            .map_err(not_given)
    })
}

/// What a person is asked for the Passphrase of the Library called `library`.
fn referencing_prompt(library: &str) -> String {
    format!(
        "The Passphrase given does not open a Library that already uses this account. \
         Enter the Passphrase of the Library {library:?}: "
    )
}

/// What the device layer is told when the terminal produced no Passphrase.
///
/// This crate's own refusal, carried whole: the device layer has nothing to add
/// to why a terminal gave no Passphrase, and a caller walking the chain reads
/// which check failed and what the terminal said under its one line.
fn not_given(cause: Error) -> coffret_device::Error {
    coffret_device::Error::PassphraseNotGiven {
        cause: Box::new(cause),
    }
}

/// Reads the Passphrase of a Library that already exists.
///
/// Once, because there is a stored form to check it against: a Passphrase typed
/// wrongly is refused by the file rather than by a second prompt.
fn enter(from_stdin: bool) -> Result<Passphrase> {
    if from_stdin {
        return read_line();
    }
    let passphrase = rpassword::prompt_password("Enter the Passphrase: ")
        .map_err(Error::unread(Secret::Passphrase, Source::Terminal))?;
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
fn choose(from_stdin: bool) -> Result<Passphrase> {
    let chosen = if from_stdin {
        read_line()?
    } else {
        choose_with(|prompt| {
            rpassword::prompt_password(prompt)
                .map_err(Error::unread(Secret::Passphrase, Source::Terminal))
        })?
    };
    if chosen.is_empty() {
        return Err(Error::EmptyPassphrase);
    }
    Ok(chosen)
}

/// Reads and compares the two terminal entries. Keeping the prompt mechanism
/// behind this small boundary lets tests exercise the interactive behavior
/// without needing a person's terminal.
fn choose_with(mut prompt: impl FnMut(&str) -> Result<String>) -> Result<Passphrase> {
    // Both readings become a `Passphrase` the moment they are read, so both are
    // wiped whatever happens next: the second is only ever compared against
    // the first, and the first is returned rather than copied into the value
    // that is.
    let chosen = taken(prompt("Choose a Passphrase: ")?);
    let again = taken(prompt("Enter it again: ")?);
    if chosen != again {
        return Err(Error::PassphrasesDiffer);
    }
    Ok(chosen)
}

/// Reads one line of standard input, without the line ending.
///
/// For a script and for a test: neither has a terminal to be prompted at, and
/// a Passphrase on the command line would sit in the shell history and in the
/// process table where anyone on the machine could read it (spec: DK-10).
fn read_line() -> Result<Passphrase> {
    let mut line = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(Error::unread(Secret::Passphrase, Source::StandardInput))?;
    if read == 0 {
        return Err(Error::InputEnded {
            secret: Secret::Passphrase,
        });
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
    use std::collections::VecDeque;

    use super::*;

    // The prompt names the Library whose Passphrase it wants, and ends in a
    // colon as the others do.
    #[test]
    fn the_referencing_prompt_names_the_library() {
        assert_eq!(
            referencing_prompt("at-work"),
            "The Passphrase given does not open a Library that already uses this account. \
             Enter the Passphrase of the Library \"at-work\": "
        );
    }

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

    #[test]
    fn choosing_interactively_reads_the_passphrase_twice() {
        let mut answers = VecDeque::from(["chosen once".to_owned(), "chosen once".to_owned()]);
        let mut prompts = Vec::new();
        let chosen = choose_with(|prompt| {
            prompts.push(prompt.to_owned());
            Ok(answers.pop_front().expect("one answer for each prompt"))
        })
        .expect("two matching answers choose a Passphrase");

        assert_eq!(chosen.as_bytes(), b"chosen once");
        assert_eq!(prompts, ["Choose a Passphrase: ", "Enter it again: "]);
        assert!(answers.is_empty());
    }

    // The one refusal the two readings can make between them. The sentence a
    // person reads over it is pinned with the others in the error module.
    #[test]
    fn two_readings_that_differ_choose_nothing() {
        let mut answers = VecDeque::from(["chosen once".to_owned(), "chosen twice".to_owned()]);
        let refused = choose_with(|_| Ok(answers.pop_front().expect("one answer for each prompt")))
            .expect_err("two different answers choose no Passphrase");

        assert!(matches!(refused, Error::PassphrasesDiffer), "{refused:?}");
    }

    // What the device layer is handed is this crate's own refusal, whole: the
    // value itself under the device's one line, not a rendering of it.
    #[test]
    fn a_passphrase_not_given_reaches_the_device_layer_whole() {
        use std::error::Error as _;

        let handed = not_given(Error::EmptyPassphrase);

        assert!(
            matches!(handed, coffret_device::Error::PassphraseNotGiven { .. }),
            "{handed:?}",
        );
        let cause = handed
            .source()
            .expect("the device layer carries what the terminal reported");
        assert!(
            matches!(cause.downcast_ref::<Error>(), Some(Error::EmptyPassphrase)),
            "the cause is this crate's own value, got {cause:?}",
        );
    }
}
