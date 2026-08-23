use std::error;
use std::fmt;
use std::path::PathBuf;

use crate::http::TransportError;

/// Result alias for this crate's own fallible surface.
pub type Result<T> = std::result::Result<T, Error>;

/// What can go wrong getting this gateway ready to serve the port.
///
/// The port's own operations answer in [`coffret_usecase::Error`]; this is the
/// layer under it — building a transport, running the authorization flow,
/// keeping the token cache — which has failures of its own that no Storage
/// vocabulary would describe honestly. Where one of these surfaces during an
/// operation it is translated, so a caller of the port still only ever sees the
/// port's error type.
#[derive(Debug)]
pub enum Error {
    /// The HTTP client could not be built.
    HttpClient {
        /// What the client library reported.
        detail: String,
    },
    /// The token cache could not be read or written.
    TokenCache {
        /// The file that was being read or written.
        path: PathBuf,
        /// What the operating system reported.
        detail: String,
    },
    /// The token cache holds something this build cannot read.
    MalformedTokenCache {
        /// The file that was read.
        path: PathBuf,
        /// What went wrong reading it.
        detail: String,
    },
    /// The tokens could not be sealed, so none of them were written.
    ///
    /// Sealing is the format layer's work and its answer travels here whole:
    /// this layer sees that the cache could not be written, not why, and
    /// naming a cause it did not observe would be a guess.
    UnsealableTokenCache {
        /// The file the sealed bytes were meant for.
        path: PathBuf,
        /// What the format layer reported.
        cause: coffret_format::Error,
    },
    /// No refresh token is cached, so there is nothing to authorize calls with.
    ///
    /// The authorization flow has to be run — which needs a person at a browser
    /// — before this store can be used again.
    NotAuthorized,
    /// The authorization flow did not complete.
    Authorization {
        /// What went wrong.
        detail: String,
    },
    /// The token endpoint refused to issue or refresh a token.
    TokenEndpoint {
        /// The status it answered with.
        status: u16,
        /// What it reported.
        detail: String,
    },
    /// A call to the token endpoint never became an answer.
    Transport(TransportError),
    /// The operating system would not supply random bytes for the PKCE
    /// verifier, so no authorization request can be made safely.
    EntropyUnavailable {
        /// What the entropy source reported.
        detail: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpClient { detail } => write!(f, "could not build an HTTP client: {detail}"),
            Self::TokenCache { path, detail } => {
                write!(f, "could not use the token cache at {path:?}: {detail}")
            }
            Self::MalformedTokenCache { path, detail } => {
                write!(f, "the token cache at {path:?} is unreadable: {detail}")
            }
            Self::UnsealableTokenCache { path, cause } => {
                write!(f, "could not seal the token cache at {path:?}: {cause}")
            }
            Self::NotAuthorized => {
                f.write_str("no refresh token is cached; run the authorization flow first")
            }
            Self::Authorization { detail } => write!(f, "authorization did not complete: {detail}"),
            Self::TokenEndpoint { status, detail } => {
                write!(f, "the token endpoint answered {status}: {detail}")
            }
            Self::Transport(error) => write!(f, "could not reach the token endpoint: {error}"),
            Self::EntropyUnavailable { detail } => {
                write!(f, "could not draw random bytes: {detail}")
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::UnsealableTokenCache { cause, .. } => Some(cause),
            _ => None,
        }
    }
}

impl From<TransportError> for Error {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

impl From<Error> for coffret_usecase::Error {
    fn from(error: Error) -> Self {
        let detail = error.to_string();
        match error {
            // Nothing about the request is wrong; there is simply no usable
            // credential, and no number of retries will produce one.
            Error::NotAuthorized
            | Error::Authorization { .. }
            | Error::TokenEndpoint { .. }
            | Error::MalformedTokenCache { .. } => Self::Unauthenticated { detail },
            Error::Transport(transport) => transport.into(),
            // Nothing is wrong with the credential or the request: the local
            // machine could not do its part of the work.
            Error::TokenCache { .. }
            | Error::UnsealableTokenCache { .. }
            | Error::EntropyUnavailable { .. } => Self::Io { detail },
            Error::HttpClient { .. } => Self::Unsupported { detail },
        }
    }
}
