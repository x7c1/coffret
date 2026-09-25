use std::error;
use std::fmt;
use std::io;
use std::str::Utf8Error;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// What can go wrong starting a coffret process.
///
/// Two kinds of thing, and nothing else: a secret that whoever started the run
/// did not give, and a log that could not be started. Each variant is a state a
/// person can be told about, and what a lower layer reported travels as the
/// value it reported — the terminal's `io::Error`, the decoder's position, the
/// log sink's own refusal — so a caller printing the chain reads that layer's
/// answer under this one's rather than a copy of it made here.
///
/// None of them repeats what was entered. A refusal of a secret names the check
/// it failed and never the characters that failed it (spec: DK-10).
#[derive(Debug)]
pub enum Error {
    /// A secret could not be read from where it was being asked for.
    Unread {
        /// Which secret was being asked for.
        secret: Secret,
        /// Where it was being read from.
        from: Source,
        /// What the terminal or the pipe reported.
        cause: io::Error,
    },
    /// Standard input ended before a secret was given.
    InputEnded {
        /// Which secret was being asked for.
        secret: Secret,
    },
    /// The two readings of a Passphrase a Library was to be created under were
    /// not the same.
    PassphrasesDiffer,
    /// The Passphrase a Library was to be created under was empty.
    EmptyPassphrase,
    /// What was entered for a Recovery Code was empty.
    EmptyRecoveryCode {
        /// Where it was read from, which is what the sentence calls it by.
        from: Source,
    },
    /// What was entered for a Recovery Code is longer than any code is.
    RecoveryCodeTooLong {
        /// Where it was read from, which is what the sentence calls it by.
        from: Source,
        /// The most bytes a Recovery Code is read to.
        ceiling: usize,
    },
    /// What was entered for a Recovery Code is not text.
    RecoveryCodeNotUtf8 {
        /// Where the decoding failed: a position and a length, and none of the
        /// bytes it failed on.
        cause: Utf8Error,
    },
    /// The settings the log is started from could not be read.
    LogSettingsUnread {
        /// What the log sink reported.
        cause: coffret_logging::Error,
    },
    /// The log could not be started.
    LogNotStarted {
        /// What the log sink reported.
        cause: coffret_logging::Error,
    },
}

/// Which secret a refusal is about.
#[derive(Debug, Clone, Copy)]
pub enum Secret {
    /// What a Library's stored Master Key is opened with.
    Passphrase,
    /// What a device joining a Library is given the Master Key by.
    RecoveryCode,
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Passphrase => "Passphrase",
            Self::RecoveryCode => "Recovery Code",
        })
    }
}

/// Where a secret was being read from.
#[derive(Debug, Clone, Copy)]
pub enum Source {
    /// A prompt that does not echo.
    Terminal,
    /// One line of standard input, where a script selected that.
    StandardInput,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // What the terminal or the pipe said is the `io::Error` underneath,
            // which a caller printing the chain reads under this line.
            Self::Unread {
                secret,
                from: Source::Terminal,
                ..
            } => write!(f, "the {secret} could not be read"),
            Self::Unread {
                secret,
                from: Source::StandardInput,
                ..
            } => write!(f, "the {secret} could not be read from standard input"),
            Self::InputEnded { secret } => {
                write!(f, "standard input ended before a {secret} was given")
            }
            Self::PassphrasesDiffer => {
                f.write_str("the two Passphrases are not the same; nothing was created")
            }
            Self::EmptyPassphrase => {
                f.write_str("an empty Passphrase protects nothing; nothing was created")
            }
            Self::EmptyRecoveryCode {
                from: Source::Terminal,
            } => f.write_str("an empty entry is not a Recovery Code"),
            Self::EmptyRecoveryCode {
                from: Source::StandardInput,
            } => f.write_str("an empty line is not a Recovery Code"),
            Self::RecoveryCodeTooLong {
                from: Source::Terminal,
                ceiling,
            } => write!(f, "the Recovery Code is longer than {ceiling} bytes"),
            Self::RecoveryCodeTooLong {
                from: Source::StandardInput,
                ceiling,
            } => write!(f, "the Recovery Code line is longer than {ceiling} bytes"),
            // Where the decoding failed is the cause underneath, for the reason
            // the terminal's answer is.
            Self::RecoveryCodeNotUtf8 { .. } => {
                f.write_str("the Recovery Code must be valid UTF-8")
            }
            Self::LogSettingsUnread { .. } => f.write_str("the log settings could not be read"),
            Self::LogNotStarted { .. } => f.write_str("logging could not be started"),
        }
    }
}

impl Error {
    /// What a terminal or a pipe that could not be read is reported as, for
    /// `map_err` at the read.
    pub(crate) fn unread(secret: Secret, from: Source) -> impl FnOnce(io::Error) -> Self {
        move |cause| Self::Unread {
            secret,
            from,
            cause,
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Unread { cause, .. } => Some(cause),
            Self::RecoveryCodeNotUtf8 { cause } => Some(cause),
            Self::LogSettingsUnread { cause } | Self::LogNotStarted { cause } => Some(cause),
            Self::InputEnded { .. }
            | Self::PassphrasesDiffer
            | Self::EmptyPassphrase
            | Self::EmptyRecoveryCode { .. }
            | Self::RecoveryCodeTooLong { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    /// The links a caller printing the chain reads, outermost first.
    fn chain(error: &dyn error::Error) -> Vec<String> {
        let mut links = vec![error.to_string()];
        let mut below = error.source();
        while let Some(link) = below {
            links.push(link.to_string());
            below = link.source();
        }
        links
    }

    // The sentences a person reads over a Passphrase that was not given, word
    // for word: they are what a terminal shows, and a rewording of any of them
    // is a change somebody should have meant.
    #[test]
    fn the_passphrase_sentences_are_the_ones_a_person_reads() {
        let said = |error: Error| error.to_string();
        assert_eq!(
            said(Error::Unread {
                secret: Secret::Passphrase,
                from: Source::Terminal,
                cause: io::Error::from(io::ErrorKind::Interrupted),
            }),
            "the Passphrase could not be read",
        );
        assert_eq!(
            said(Error::Unread {
                secret: Secret::Passphrase,
                from: Source::StandardInput,
                cause: io::Error::from(io::ErrorKind::Interrupted),
            }),
            "the Passphrase could not be read from standard input",
        );
        assert_eq!(
            said(Error::InputEnded {
                secret: Secret::Passphrase,
            }),
            "standard input ended before a Passphrase was given",
        );
        assert_eq!(
            said(Error::PassphrasesDiffer),
            "the two Passphrases are not the same; nothing was created",
        );
        assert_eq!(
            said(Error::EmptyPassphrase),
            "an empty Passphrase protects nothing; nothing was created",
        );
    }

    // The same, for the Recovery Code: the sentence names the check that
    // failed, and where the entry came from decides what it is called.
    #[test]
    fn the_recovery_code_sentences_are_the_ones_a_person_reads() {
        let said = |error: Error| error.to_string();
        assert_eq!(
            said(Error::Unread {
                secret: Secret::RecoveryCode,
                from: Source::Terminal,
                cause: io::Error::from(io::ErrorKind::Interrupted),
            }),
            "the Recovery Code could not be read",
        );
        assert_eq!(
            said(Error::Unread {
                secret: Secret::RecoveryCode,
                from: Source::StandardInput,
                cause: io::Error::from(io::ErrorKind::Interrupted),
            }),
            "the Recovery Code could not be read from standard input",
        );
        assert_eq!(
            said(Error::InputEnded {
                secret: Secret::RecoveryCode,
            }),
            "standard input ended before a Recovery Code was given",
        );
        assert_eq!(
            said(Error::EmptyRecoveryCode {
                from: Source::Terminal,
            }),
            "an empty entry is not a Recovery Code",
        );
        assert_eq!(
            said(Error::EmptyRecoveryCode {
                from: Source::StandardInput,
            }),
            "an empty line is not a Recovery Code",
        );
        assert_eq!(
            said(Error::RecoveryCodeTooLong {
                from: Source::Terminal,
                ceiling: 256,
            }),
            "the Recovery Code is longer than 256 bytes",
        );
        assert_eq!(
            said(Error::RecoveryCodeTooLong {
                from: Source::StandardInput,
                ceiling: 256,
            }),
            "the Recovery Code line is longer than 256 bytes",
        );
    }

    // What the terminal said is the next link rather than part of this one, so
    // a chain printed link by link says each thing once.
    #[test]
    fn what_the_terminal_reported_is_the_next_link_and_is_said_once() {
        let error = Error::Unread {
            secret: Secret::Passphrase,
            from: Source::Terminal,
            cause: io::Error::other("the terminal went away"),
        };

        assert_eq!(
            chain(&error),
            vec![
                "the Passphrase could not be read".to_owned(),
                "the terminal went away".to_owned(),
            ],
        );
        assert!(
            error
                .source()
                .and_then(|cause| cause.downcast_ref::<io::Error>())
                .is_some(),
            "the cause is the terminal's own value and not a rendering of it",
        );
    }

    // The log sink's refusal travels as its own value for the same reason.
    #[test]
    fn a_log_that_could_not_be_started_carries_what_the_sink_reported() {
        let error = Error::LogNotStarted {
            cause: coffret_logging::Error::AlreadyInstalled,
        };

        let links = chain(&error);
        assert_eq!(links[0], "logging could not be started");
        assert_eq!(
            links[1],
            coffret_logging::Error::AlreadyInstalled.to_string()
        );
        assert!(
            error
                .source()
                .and_then(|cause| cause.downcast_ref::<coffret_logging::Error>())
                .is_some(),
            "the cause is the sink's own value",
        );
        assert_eq!(
            Error::LogSettingsUnread {
                cause: coffret_logging::Error::NoStateDirectory,
            }
            .to_string(),
            "the log settings could not be read",
        );
    }
}
