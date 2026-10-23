use std::error;
use std::fmt;
use std::io;
use std::num::ParseIntError;

use crate::{Drawn, IoStep, IDLE_MINUTES};

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// What can refuse a desktop shell's call.
///
/// Each variant is one sentence a person can be told, and what a lower layer
/// reported travels as the value it reported, so that the shell, printing the
/// chain, reads that layer's answer under this one's. The server's own failures
/// arrive as the chain the server builds ([`Error::Server`]); every other cause
/// is kept by its type, which is what lets [`refusal_line()`] and
/// [`refusal_evidence()`] answer by matching on the variant rather than by
/// searching a chain.
///
/// [`refusal_line()`]: crate::refusal_line()
/// [`refusal_evidence()`]: crate::refusal_evidence()
#[derive(Debug)]
pub enum Error {
    /// The process's desktop was started a second time.
    AlreadyStarted,
    /// What [`IDLE_MINUTES`] was set to is not text.
    IdleMinutesNotText,
    /// What [`IDLE_MINUTES`] was set to is not a whole number.
    IdleMinutesNotWhole {
        /// What it was set to.
        text: String,
        /// What reading it as a number reported.
        cause: ParseIntError,
    },
    /// [`IDLE_MINUTES`] was set to less than one minute.
    IdleMinutesBelowOne {
        /// What it was set to.
        minutes: u64,
    },
    /// The log could not be started.
    LogNotStarted {
        /// What starting it reported.
        cause: coffret_shell::Error,
    },
    /// The Libraries on this device could not be listed.
    LibrariesUnlisted {
        /// What the device reported.
        cause: coffret_device::Error,
    },
    /// A call that needs an open Library came while none was open.
    NothingOpen,
    /// A Passphrase window for an unlock was asked for while nothing was open.
    NothingToUnlock,
    /// A random value could not be drawn.
    NotDrawn {
        /// What it was drawn for.
        what: Drawn,
        /// What the source of randomness reported.
        cause: coffret_explorer_host::credentials::NotDrawn,
    },
    /// The one-time handle for a drop could not be drawn.
    DropHandleNotDrawn {
        /// What the source of randomness reported.
        cause: getrandom::Error,
    },
    /// A socket or the runtime could not be set up.
    Io {
        /// Which step failed.
        step: IoStep,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// The device refused, as when the Passphrase does not open the Library or
    /// the Library is not on this device.
    Device(coffret_device::Error),
    /// The explorer's host could not be built.
    Host(coffret_explorer_host::Error),
    /// The server refused to open, to unlock or to go on serving, with the
    /// chain it says that in.
    Server(anyhow::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => f.write_str("the desktop has already started"),
            Self::IdleMinutesNotText => write!(f, "{IDLE_MINUTES} is not text"),
            Self::IdleMinutesNotWhole { text, .. } => {
                write!(
                    f,
                    "{IDLE_MINUTES} is not a whole number of minutes: {text:?}"
                )
            }
            Self::IdleMinutesBelowOne { minutes } => {
                write!(f, "{IDLE_MINUTES} must be at least 1, not {minutes}")
            }
            Self::LogNotStarted { .. } => f.write_str("the log could not be started"),
            Self::LibrariesUnlisted { .. } => {
                f.write_str("the Libraries on this device could not be listed")
            }
            Self::NothingOpen => f.write_str("no Library is open in this app"),
            Self::NothingToUnlock => f.write_str("no Library is open in this app to unlock"),
            Self::NotDrawn { what, .. } => what.fmt(f),
            Self::DropHandleNotDrawn { .. } => f.write_str("a drop handle could not be drawn"),
            Self::Io { step, .. } => step.fmt(f),
            Self::Device(error) => error.fmt(f),
            Self::Host(error) => error.fmt(f),
            Self::Server(error) => error.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::AlreadyStarted
            | Self::IdleMinutesNotText
            | Self::IdleMinutesBelowOne { .. }
            | Self::NothingOpen
            | Self::NothingToUnlock => None,
            Self::IdleMinutesNotWhole { cause, .. } => Some(cause),
            Self::LogNotStarted { cause } => Some(cause),
            Self::LibrariesUnlisted { cause } => Some(cause),
            Self::NotDrawn { cause, .. } => Some(cause),
            Self::DropHandleNotDrawn { cause } => Some(cause),
            Self::Io { cause, .. } => Some(cause),
            // The device's, the host's and the server's own errors are this
            // one's sentence, so what comes under them is what comes under
            // this.
            Self::Device(error) => error.source(),
            Self::Host(error) => error.source(),
            Self::Server(error) => error.source(),
        }
    }
}

impl From<coffret_device::Error> for Error {
    fn from(error: coffret_device::Error) -> Self {
        Self::Device(error)
    }
}

impl From<coffret_explorer_host::Error> for Error {
    fn from(error: coffret_explorer_host::Error) -> Self {
        Self::Host(error)
    }
}

impl Error {
    /// The whole chain, each cause after the one it is under: the sentence a
    /// shell shows where it shows one, read the way `anyhow` renders `{:#}`.
    pub fn chain_line(&self) -> String {
        let mut line = self.to_string();
        let mut cause = error::Error::source(self);
        while let Some(next) = cause {
            line.push_str(": ");
            line.push_str(&next.to_string());
            cause = next.source();
        }
        line
    }
}
