use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

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
///
/// A failure this layer observed as a Rust error travels as that error: the
/// value goes in a `cause`, and only the port boundary below turns one into the
/// port's own vocabulary. What a *remote* reported — a status, a body, a
/// message from the token endpoint — is text where it arrived as text, and
/// stays text.
#[derive(Debug)]
pub enum Error {
    /// The HTTP client could not be built.
    HttpClient {
        /// What the client library reported.
        cause: reqwest::Error,
    },
    /// The token cache could not be read or written.
    TokenCache {
        /// The file that was being read or written.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// The token cache holds something this build cannot read.
    MalformedTokenCache {
        /// The file that was read.
        path: PathBuf,
        /// What went wrong reading it.
        cause: TokenCacheDefect,
    },
    /// The tokens could not be encoded, so none of them were written.
    ///
    /// This is the step before sealing: the tokens are turned into the document
    /// that gets sealed, and a document that cannot be produced is not the file
    /// layer's failure and not the operating system's.
    UnencodableTokens {
        /// The file the tokens were meant for.
        path: PathBuf,
        /// What the encoder reported.
        cause: serde_json::Error,
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
    ///
    /// What this carries is what the flow itself, or the person's browser,
    /// said: a grant that reached too far, a request that was refused, a
    /// redirect that never came. Where a Rust error is what went wrong,
    /// [`Error::LoopbackRedirect`] or [`Error::MalformedRedirect`] carries it.
    Authorization {
        /// What went wrong.
        detail: String,
    },
    /// The loopback the browser is redirected back to could not be run.
    ///
    /// Listening on the port, learning which one the operating system handed
    /// out, taking the browser's connection, reading what it asked for: all of
    /// it is this machine's own work, and what stopped it is what the operating
    /// system reported.
    LoopbackRedirect {
        /// The step that failed.
        step: RedirectStep,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// The browser came back asking for something that is not a URL.
    MalformedRedirect {
        /// What it asked for.
        target: String,
        /// What the parser reported.
        cause: url::ParseError,
    },
    /// The token endpoint refused to issue or refresh a token.
    TokenEndpoint {
        /// The status it answered with.
        status: u16,
        /// What it reported.
        detail: String,
    },
    /// The token endpoint answered, and the answer could not be read.
    ///
    /// Apart from [`Error::TokenEndpoint`] because the two are told apart by
    /// who observed the failure: there the endpoint said what was wrong, here
    /// nothing it said was ever recovered and what went wrong is a Rust error
    /// this layer saw. The status is still worth carrying — it says what kind
    /// of answer was being read.
    UnreadableTokenResponse {
        /// The status it answered with.
        status: u16,
        /// What went wrong reading the answer.
        cause: TokenResponseDefect,
    },
    /// A call to the token endpoint never became an answer.
    Transport(TransportError),
    /// The operating system would not supply random bytes for the PKCE
    /// verifier, so no authorization request can be made safely.
    EntropyUnavailable {
        /// What the entropy source reported.
        cause: getrandom::Error,
    },
}

/// What made a cached token file unreadable.
///
/// The two are one verdict to a caller — the cache is no good, so authorize
/// again — and are kept apart only so that whichever layer saw the failure has
/// its own answer travel whole, rather than a message composed here about work
/// this layer did not do.
#[derive(Debug)]
pub enum TokenCacheDefect {
    /// The sealed form could not be opened: another Master Key wrote it, its
    /// bytes have been edited, or it was never a sealed cache at all.
    Sealed(coffret_format::Error),
    /// The sealed form opened, and what was inside is not the token document
    /// this build expects.
    Document(serde_json::Error),
}

impl fmt::Display for TokenCacheDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sealed(cause) => write!(f, "{cause}"),
            Self::Document(cause) => write!(f, "{cause}"),
        }
    }
}

impl error::Error for TokenCacheDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Sealed(cause) => Some(cause),
            Self::Document(cause) => Some(cause),
        }
    }
}

/// What kept the token endpoint's answer from being read.
///
/// As with [`TokenCacheDefect`], the two are one verdict to a caller — nothing
/// usable came back from the endpoint — and are kept apart so that whichever
/// layer saw the failure has its own answer travel whole.
#[derive(Debug)]
pub enum TokenResponseDefect {
    /// The body never arrived whole: the transfer broke, or it was not as
    /// long as the answer declared.
    Body(coffret_usecase::Error),
    /// The body arrived, and what it holds is not the token document this
    /// build expects.
    Document(serde_json::Error),
}

impl fmt::Display for TokenResponseDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Body(cause) => write!(f, "{cause}"),
            Self::Document(cause) => write!(f, "{cause}"),
        }
    }
}

impl error::Error for TokenResponseDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Body(cause) => Some(cause),
            Self::Document(cause) => Some(cause),
        }
    }
}

/// Which call of the loopback redirect a failure happened at.
///
/// The operating system reports what went wrong, never what was being asked
/// of it, and that is what separates a port already taken from a browser that
/// hung up before it said anything.
#[derive(Debug)]
pub enum RedirectStep {
    /// Listening on the loopback port the browser is to be sent back to.
    Bind,
    /// Reading back which port the operating system handed out.
    Port,
    /// Taking the connection the browser arrives on.
    Accept,
    /// Reading the request the browser sent.
    Read,
}

impl fmt::Display for RedirectStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Bind => "could not listen for the redirect",
            Self::Port => "could not read the redirect port",
            Self::Accept => "could not accept the redirect",
            Self::Read => "could not read the redirect",
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpClient { cause } => write!(f, "could not build an HTTP client: {cause}"),
            Self::TokenCache { path, cause } => {
                write!(f, "could not use the token cache at {path:?}: {cause}")
            }
            Self::MalformedTokenCache { path, cause } => {
                write!(f, "the token cache at {path:?} is unreadable: {cause}")
            }
            Self::UnencodableTokens { path, cause } => {
                write!(
                    f,
                    "could not encode the tokens for the cache at {path:?}: {cause}"
                )
            }
            Self::UnsealableTokenCache { path, cause } => {
                write!(f, "could not seal the token cache at {path:?}: {cause}")
            }
            Self::NotAuthorized => {
                f.write_str("no refresh token is cached; run the authorization flow first")
            }
            Self::Authorization { detail } => write!(f, "authorization did not complete: {detail}"),
            Self::LoopbackRedirect { step, cause } => {
                write!(f, "authorization did not complete: {step}: {cause}")
            }
            Self::MalformedRedirect { target, cause } => {
                write!(
                    f,
                    "authorization did not complete: the redirect target {target:?} is not a URL: {cause}"
                )
            }
            Self::TokenEndpoint { status, detail } => {
                write!(f, "the token endpoint answered {status}: {detail}")
            }
            Self::UnreadableTokenResponse { status, cause } => {
                write!(
                    f,
                    "the token endpoint answered {status}: unreadable token response: {cause}"
                )
            }
            Self::Transport(error) => write!(f, "could not reach the token endpoint: {error}"),
            Self::EntropyUnavailable { cause } => {
                write!(f, "could not draw random bytes: {cause}")
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::HttpClient { cause } => Some(cause),
            Self::TokenCache { cause, .. } => Some(cause),
            Self::MalformedTokenCache { cause, .. } => Some(cause),
            Self::UnencodableTokens { cause, .. } => Some(cause),
            Self::UnsealableTokenCache { cause, .. } => Some(cause),
            Self::UnreadableTokenResponse { cause, .. } => Some(cause),
            Self::LoopbackRedirect { cause, .. } => Some(cause),
            Self::MalformedRedirect { cause, .. } => Some(cause),
            Self::Transport(error) => Some(error),
            Self::EntropyUnavailable { cause } => Some(cause),
            // Nothing a Rust error reported: what these carry is what a remote
            // said, or that nothing was cached at all.
            Self::NotAuthorized | Self::Authorization { .. } | Self::TokenEndpoint { .. } => None,
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
            | Error::LoopbackRedirect { .. }
            | Error::MalformedRedirect { .. }
            | Error::TokenEndpoint { .. }
            | Error::UnreadableTokenResponse { .. }
            | Error::MalformedTokenCache { .. } => Self::Unauthenticated { detail },
            Error::Transport(transport) => transport.into(),
            // Nothing is wrong with the credential or the request: the local
            // machine could not do its part of the work. The port carries an
            // `io::Error`, so the kind the operating system reported is kept —
            // it is what a caller acts on — while the message this layer
            // composed, which names the file, becomes that error's own.
            Error::TokenCache { cause, .. } => Self::Io {
                cause: Arc::new(io::Error::new(cause.kind(), detail)),
            },
            // Local failures the operating system was never asked about: the
            // port names every failure of this machine's own part `Io`, and
            // there is no kind to keep.
            Error::UnencodableTokens { .. }
            | Error::UnsealableTokenCache { .. }
            | Error::EntropyUnavailable { .. } => Self::Io {
                cause: Arc::new(io::Error::other(detail)),
            },
            Error::HttpClient { .. } => Self::Unsupported { detail },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `serde_json::Error` like the one a failed encode would hand over.
    fn json_error() -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>("{").expect_err("the document is truncated")
    }

    fn path() -> PathBuf {
        PathBuf::from("/home/someone/.config/coffret/tokens.bin")
    }

    #[test]
    fn a_refused_cache_file_reaches_the_port_carrying_its_kind() {
        let error = Error::TokenCache {
            path: path(),
            cause: io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied"),
        };
        let message = error.to_string();

        let coffret_usecase::Error::Io { cause } = coffret_usecase::Error::from(error) else {
            panic!("a cache the operating system refused is a local failure");
        };
        assert_eq!(cause.kind(), io::ErrorKind::PermissionDenied);
        // The file it happened to is in the message, where a reader needs it.
        assert_eq!(cause.to_string(), message);
    }

    // Tokens that cannot be encoded are this machine's failure, not a reason to
    // send anybody to look at their authorization.
    #[test]
    fn tokens_that_cannot_be_encoded_reach_the_port_as_a_local_failure() {
        let error = Error::UnencodableTokens {
            path: path(),
            cause: json_error(),
        };

        assert!(error::Error::source(&error).is_some());
        assert!(matches!(
            coffret_usecase::Error::from(error),
            coffret_usecase::Error::Io { .. }
        ));
    }

    #[test]
    fn either_defect_in_a_cache_reaches_the_port_as_unauthenticated() {
        let defects = [
            TokenCacheDefect::Sealed(coffret_format::Error::AuthenticationFailed),
            TokenCacheDefect::Document(json_error()),
        ];
        for cause in defects {
            let error = Error::MalformedTokenCache {
                path: path(),
                cause,
            };
            assert!(error::Error::source(&error).is_some());
            assert!(matches!(
                coffret_usecase::Error::from(error),
                coffret_usecase::Error::Unauthenticated { .. }
            ));
        }
    }

    #[test]
    fn either_defect_in_an_answer_reaches_the_port_as_unauthenticated() {
        let defects = [
            TokenResponseDefect::Body(coffret_usecase::Error::LengthMismatch {
                expected: 64,
                actual: 10,
            }),
            TokenResponseDefect::Document(json_error()),
        ];
        for cause in defects {
            let error = Error::UnreadableTokenResponse { status: 200, cause };
            assert!(error.to_string().contains("answered 200"), "{error}");
            assert!(error::Error::source(&error).is_some());
            assert!(matches!(
                coffret_usecase::Error::from(error),
                coffret_usecase::Error::Unauthenticated { .. }
            ));
        }
    }

    #[test]
    fn a_loopback_that_will_not_run_reaches_the_port_as_unauthenticated() {
        let steps = [
            (RedirectStep::Bind, "could not listen for the redirect"),
            (RedirectStep::Port, "could not read the redirect port"),
            (RedirectStep::Accept, "could not accept the redirect"),
            (RedirectStep::Read, "could not read the redirect"),
        ];
        for (step, said) in steps {
            let error = Error::LoopbackRedirect {
                step,
                cause: io::Error::new(io::ErrorKind::AddrInUse, "Address already in use"),
            };
            assert_eq!(
                error.to_string(),
                format!("authorization did not complete: {said}: Address already in use")
            );
            assert!(error::Error::source(&error).is_some());
            assert!(matches!(
                coffret_usecase::Error::from(error),
                coffret_usecase::Error::Unauthenticated { .. }
            ));
        }
    }

    #[test]
    fn a_redirect_target_that_is_not_a_url_reaches_the_port_as_unauthenticated() {
        let target = ":99999999";
        let cause = url::Url::parse(&format!("http://127.0.0.1{target}"))
            .expect_err("no port is that large");
        let error = Error::MalformedRedirect {
            target: target.to_owned(),
            cause,
        };

        // What the browser asked for is quoted, so a target with whitespace or
        // control bytes in it is still readable in a log.
        assert!(
            error.to_string().contains(&format!("{target:?}")),
            "{error}"
        );
        assert!(error::Error::source(&error).is_some());
        assert!(matches!(
            coffret_usecase::Error::from(error),
            coffret_usecase::Error::Unauthenticated { .. }
        ));
    }

    // A client that cannot be built is not something a retry or a fresh
    // authorization would help with: this build asked for something the library
    // cannot do.
    #[test]
    fn a_client_that_cannot_be_built_reaches_the_port_as_unsupported() {
        // No pair of TLS versions is both at least 1.3 and at most 1.2, so the
        // builder refuses without a network being involved.
        let cause = reqwest::Client::builder()
            .min_tls_version(reqwest::tls::Version::TLS_1_3)
            .max_tls_version(reqwest::tls::Version::TLS_1_2)
            .build()
            .expect_err("no TLS version satisfies both bounds");
        let error = Error::HttpClient { cause };

        assert!(error::Error::source(&error).is_some());
        assert!(matches!(
            coffret_usecase::Error::from(error),
            coffret_usecase::Error::Unsupported { .. }
        ));
    }

    #[test]
    fn an_entropy_source_that_will_not_answer_reaches_the_port_as_a_local_failure() {
        let error = Error::EntropyUnavailable {
            cause: getrandom::Error::UNSUPPORTED,
        };

        assert!(error::Error::source(&error).is_some());
        assert!(matches!(
            coffret_usecase::Error::from(error),
            coffret_usecase::Error::Io { .. }
        ));
    }
}
