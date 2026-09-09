//! Reading a Recovery Code without putting the Master Key in process arguments
//! (spec: DK-10).
//!
//! A code is taken from a non-echoing prompt, or from one bounded line of
//! standard input where a script selected that explicitly — and never from a
//! command-line argument, so nothing of it reaches the process's argument list
//! or a shell history. A refusal names the check that failed and never repeats
//! what was entered.

use std::io::{BufRead, Read};

use anyhow::{bail, Context};
use zeroize::{Zeroize, Zeroizing};

/// More than enough room for the 80-character canonical form and its printed
/// grouping, while putting a firm ceiling on input controlled by a pipe.
const MAX_LINE_BYTES: usize = 256;

/// The most bytes taken from standard input: the longest line still acceptable,
/// plus room for a CRLF ending. A read that stops here is either a whole line
/// or already over the ceiling, so nothing is silently truncated into a shorter
/// code. The buffer is reserved to it up front, so no reallocation leaves a
/// copy of the code in freed memory.
const READ_LIMIT: usize = MAX_LINE_BYTES + 2;

/// What `coffret-device` is handed to ask for a Recovery Code.
///
/// A callback keeps the secret unread until the device layer has rejected an
/// unusable local name or provider location. The device layer also owns the
/// error vocabulary the rest of the join reports.
pub fn entering(from_stdin: bool) -> impl FnOnce() -> coffret_device::Result<Zeroizing<String>> {
    move || enter(from_stdin).map_err(not_given)
}

fn not_given(cause: anyhow::Error) -> coffret_device::Error {
    coffret_device::Error::RecoveryCodeNotGiven {
        cause: cause.into(),
    }
}

fn enter(from_stdin: bool) -> anyhow::Result<Zeroizing<String>> {
    if from_stdin {
        return read_line(&mut std::io::stdin().lock());
    }

    prompt(rpassword::ConfigBuilder::new().build())
}

fn prompt(config: rpassword::Config) -> anyhow::Result<Zeroizing<String>> {
    let entered = rpassword::prompt_password_with_config("Enter the Recovery Code: ", config)
        .context("the Recovery Code could not be read")?;
    bounded(entered)
}

/// Reads at most one bounded line and leaves the following line for another
/// secret consumer, which is the Passphrase reader during scripted joins.
fn read_line(reader: &mut impl BufRead) -> anyhow::Result<Zeroizing<String>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(READ_LIMIT));
    let read = reader
        .take(READ_LIMIT as u64)
        .read_until(b'\n', &mut bytes)
        .context("the Recovery Code could not be read from standard input")?;
    if read == 0 {
        bail!("standard input ended before a Recovery Code was given");
    }
    while matches!(bytes.last(), Some(b'\r' | b'\n')) {
        bytes.pop();
    }
    if bytes.is_empty() {
        bail!("an empty line is not a Recovery Code");
    }
    if bytes.len() > MAX_LINE_BYTES {
        bail!("the Recovery Code line is longer than {MAX_LINE_BYTES} bytes");
    }
    match String::from_utf8(bytes.to_vec()) {
        Ok(entered) => Ok(Zeroizing::new(entered)),
        Err(error) => {
            // Where the decoding failed is worth keeping and costs nothing:
            // `Utf8Error` is a position and a length, while the bytes it was
            // read from stay behind in the `FromUtf8Error` that is wiped here.
            let invalid = error.utf8_error();
            let mut bytes = error.into_bytes();
            bytes.zeroize();
            Err(anyhow::Error::new(invalid).context("the Recovery Code must be valid UTF-8"))
        }
    }
}

fn bounded(mut entered: String) -> anyhow::Result<Zeroizing<String>> {
    if entered.is_empty() {
        bail!("an empty entry is not a Recovery Code");
    }
    if entered.len() > MAX_LINE_BYTES {
        entered.zeroize();
        bail!("the Recovery Code is longer than {MAX_LINE_BYTES} bytes");
    }
    Ok(Zeroizing::new(entered))
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor, Write};
    use std::sync::{Arc, Mutex};

    use rpassword::ConfigBuilder;

    use super::*;

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("capture lock is available")
                .write(bytes)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    // DK-10: the prompt does not echo. All the terminal was shown is the
    // question, and none of the answer.
    #[test]
    fn the_terminal_prompt_does_not_echo_the_recovery_code() {
        let secret = "coffret1the-recovery-code-is-secret";
        let output = Captured::default();
        let config = ConfigBuilder::new()
            .input_data(format!("{secret}\n"))
            .output_writer(output.clone())
            .build();

        let entered = prompt(config).expect("the configured terminal input is readable");
        assert_eq!(entered.as_str(), secret);
        let shown = String::from_utf8(output.0.lock().expect("capture lock is available").clone())
            .expect("the prompt is UTF-8");
        assert_eq!(shown, "Enter the Recovery Code: ");
        assert!(!shown.contains(secret));
    }

    // DK-10: the reader of the first line takes exactly one line, so the
    // Passphrase on the next one is still there for the reader after it.
    #[test]
    fn a_stdin_read_takes_only_the_first_line() {
        let mut input = Cursor::new(b"the-code\nthe-passphrase\n");
        assert_eq!(read_line(&mut input).unwrap().as_str(), "the-code");

        let mut passphrase = String::new();
        input.read_line(&mut passphrase).unwrap();
        assert_eq!(passphrase, "the-passphrase\n");
    }

    // DK-10: the line is bounded, and a refusal says which check failed
    // without repeating what was entered.
    #[test]
    fn eof_and_an_overlong_line_are_clear_without_repeating_input() {
        let mut empty = Cursor::new(Vec::<u8>::new());
        assert_eq!(
            read_line(&mut empty).unwrap_err().to_string(),
            "standard input ended before a Recovery Code was given"
        );

        let mut missing = Cursor::new(b"\n");
        assert_eq!(
            read_line(&mut missing).unwrap_err().to_string(),
            "an empty line is not a Recovery Code"
        );

        let secret = "s".repeat(MAX_LINE_BYTES + 1);
        let mut input = Cursor::new(secret.as_bytes());
        let error = read_line(&mut input).unwrap_err().to_string();
        assert!(error.contains("longer than 256 bytes"), "{error}");
        assert!(!error.contains(&secret), "{error}");
    }

    // DK-10 again, for input that is not text at all: what the refusal carries
    // is where the decoding failed, and none of the bytes it failed on.
    #[test]
    fn a_line_that_is_not_utf_8_is_refused_with_where_decoding_failed() {
        let mut input = Cursor::new(b"cof\xffret\n");
        let error = read_line(&mut input).unwrap_err();
        assert_eq!(error.to_string(), "the Recovery Code must be valid UTF-8");

        let position = error
            .chain()
            .nth(1)
            .expect("the decoding failure travels underneath")
            .to_string();
        assert!(position.contains("index 3"), "{position}");
        assert!(!position.contains("ret"), "{position}");
    }
}
