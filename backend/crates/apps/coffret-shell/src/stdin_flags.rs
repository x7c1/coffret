//! The flags that say a secret is read from standard input, and never carry one
//! (spec: DK-10).
//!
//! `--recovery-code-stdin` and `--passphrase-stdin` take no value: what they
//! name is a secret, and a secret in a command line is in the process's argument
//! list, in whatever a shell keeps of its history, and in the output of any
//! `ps`. So the readers in [`recovery_code`](crate::recovery_code) and
//! [`passphrase`](crate::passphrase) take it from one bounded line of standard
//! input instead.
//!
//! A script migrating from an older spelling may still write
//! `--recovery-code-stdin <code>`. Left alone, clap meets that value as an
//! argument it was not expecting and refuses it by quoting it — and what it
//! would be quoting is the Recovery Code, put on standard error and into
//! whatever collects it. So the arguments are read for that shape before clap
//! sees them, and the refusal here says what the flag is instead of what was
//! typed after it.

use std::ffi::OsStr;

/// The flags that take no value, and the name of the secret each one reads.
const SECRET_FLAGS: [(&str, &str); 2] = [
    ("--recovery-code-stdin", "the Recovery Code"),
    ("--passphrase-stdin", "the Passphrase"),
];

/// The refusal a command line that typed a value after one of those flags
/// deserves, and `None` for one that did not.
///
/// Never the value. It is the one thing this is here to keep out of the
/// terminal, and a message that quoted it to be helpful would be the leak this
/// prevents.
///
/// `--` ends the scan: everything past it is a positional by the caller's own
/// say-so, and a folder named like a flag is not a secret about to be echoed.
pub fn value_typed_after_a_secret_flag<I, T>(arguments: I) -> Option<String>
where
    I: IntoIterator<Item = T>,
    T: AsRef<OsStr>,
{
    let mut arguments = arguments.into_iter().skip(1).peekable();
    while let Some(argument) = arguments.next() {
        // Lossy on purpose: a flag is ASCII, so anything that does not decode
        // cleanly is not one of these, and the bytes are never printed.
        let argument = argument.as_ref().to_string_lossy().into_owned();
        if argument == "--" {
            return None;
        }
        let Some((flag, secret)) = SECRET_FLAGS
            .iter()
            .find(|(flag, _)| argument == *flag || argument.starts_with(&format!("{flag}=")))
        else {
            continue;
        };
        // `--flag=value` carries it in the same token; `--flag value` carries it
        // in the next one, and a next one starting with `-` is another flag
        // rather than a value.
        let carried = argument != *flag
            || arguments
                .peek()
                .is_some_and(|next| !next.as_ref().to_string_lossy().starts_with('-'));
        if carried {
            return Some(refusal(flag, secret));
        }
    }
    None
}

/// What is said instead of what was typed.
///
/// It ends by calling the secret already typed one that has been seen, which is
/// the part a person has to act on and the part nothing else will tell them: a
/// refusal that read as a usage error would leave a Recovery Code that has been
/// seen being treated as one that has not.
fn refusal(flag: &str, secret: &str) -> String {
    format!(
        "{flag} takes no value: {secret} is read from one line of standard input, so that it \
         never reaches this process's arguments or a shell's history — give {secret} on standard \
         input and write {flag} on its own. By the time this refusal could run, what was typed \
         after the flag had already reached both of those, and refusing the run does not take it \
         back out of either, so treat {secret} as having been seen"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a leak would have put on a terminal. Nothing else in the suite
    /// spells it, so finding it anywhere is finding it having escaped.
    const TYPED: &str = "coffret1-sentinel-3f9a7c21-never-echoed";

    // DK-10: the value is refused, and the refusal is about the flag.
    #[test]
    fn a_value_after_a_secret_flag_is_refused_without_being_repeated() {
        for flag in ["--recovery-code-stdin", "--passphrase-stdin"] {
            let joined = format!("{flag}={TYPED}");
            for arguments in [
                vec!["coffret", "join", "--name", "alpha", flag, TYPED],
                vec!["coffret", "join", "--name", "alpha", flag, TYPED, "--s3"],
                vec!["coffret", "join", joined.as_str()],
            ] {
                let said = value_typed_after_a_secret_flag(&arguments)
                    .unwrap_or_else(|| panic!("a value after {flag} is refused: {arguments:?}"));

                assert!(!said.contains(TYPED), "{said}");
                assert!(said.contains(flag), "{said}");
                assert!(said.contains("takes no value"), "{said}");
                assert!(said.contains("standard input"), "{said}");
                // And that the secret already typed is one that has been seen:
                // the run is refused, but the arguments and the shell's record
                // of them are not undone, and nothing else says so.
                assert!(said.contains("having been seen"), "{said}");
            }
        }
    }

    // The shape a script is meant to use, and the two flags together, which is
    // how a join reads both secrets from the two lines it is given.
    #[test]
    fn a_flag_on_its_own_is_what_these_are_for() {
        assert_eq!(
            value_typed_after_a_secret_flag([
                "coffret",
                "join",
                "--name",
                "alpha",
                "--recovery-code-stdin",
                "--passphrase-stdin",
            ]),
            None,
        );
        assert_eq!(
            value_typed_after_a_secret_flag(["coffret", "sync", "--passphrase-stdin"]),
            None,
        );
        // The program's own name is not an argument, and one that happened to
        // be spelled like a flag is still not one.
        assert_eq!(
            value_typed_after_a_secret_flag(["--passphrase-stdin", "sync"]),
            None,
        );
    }

    // Past `--` the caller has said what the rest is, and a folder named like a
    // flag is not a secret about to be echoed.
    #[test]
    fn nothing_past_the_end_of_options_is_read_as_a_flag() {
        assert_eq!(
            value_typed_after_a_secret_flag([
                "coffret",
                "map",
                "--library",
                "alpha",
                "--",
                "--passphrase-stdin",
                "/home/someone/albums",
            ]),
            None,
        );
    }
}
