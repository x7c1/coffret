use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use coffret_format::Purpose;

use crate::http::TransportError;
use crate::oauth::{GrantedScopes, DRIVE_FILE_SCOPE};

/// Result alias for this crate's own fallible surface.
pub type Result<T> = std::result::Result<T, Error>;

/// What can go wrong getting this gateway ready to serve the port.
///
/// The port's own operations answer in [`coffret_usecase::Error`]; this is the
/// layer under it — building a transport, running the authorization flow,
/// keeping the token cache — which has failures of its own that no Storage
/// vocabulary would describe honestly. Where one of these surfaces during an
/// operation it is classified into the port's vocabulary, so a caller of the
/// port still only ever sees the port's error type.
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
    /// The key the cache was handed was derived for another purpose, so the
    /// file was never opened (spec: KD-4).
    ///
    /// Apart from [`Error::MalformedTokenCache`] because the two are facts
    /// about different things: there the file is no good, here the caller
    /// brought the wrong key and the file is very likely fine. Reporting this
    /// as a malformed cache would send somebody to delete a credential store
    /// and authorize again over a mistake one line up the call stack.
    WrongTokenCacheKey {
        /// The file the key would have been used on.
        path: PathBuf,
        /// The purpose the key was derived for.
        actual: Purpose,
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
    /// The provider answered the redirect saying it would not authorize.
    ///
    /// A redirect that names a refusal ends the wait rather than being waited
    /// out (spec: SA-2), once its `state` says the redirect is this flow's own
    /// — anything on the machine can aim an `error` at the loopback port, and
    /// only that check makes this the provider talking rather than
    /// [`Error::RedirectWithoutState`]. What the parameter holds is the
    /// provider's own word for why it said no — `access_denied` where the
    /// person declined — and it arrived as text, so it stays text.
    ProviderRefusedAuthorization {
        /// What the provider named as its reason.
        refusal: String,
    },
    /// The redirect did not carry the state this flow sent, so nothing says it
    /// is this flow's own callback.
    ///
    /// The `state` is what tells the browser coming back from the consent
    /// screen apart from any other page on the machine aimed at the loopback
    /// port, which makes this the CSRF check failing rather than a flow that
    /// merely went wrong (spec: SA-2).
    RedirectWithoutState,
    /// No redirect arrived before the flow stopped waiting.
    ///
    /// The person never finished at their browser, or what they finished in
    /// never came back to this machine. Nothing about the request was refused
    /// — nothing answered at all.
    RedirectTimedOut {
        /// How long the flow waited.
        after: Duration,
    },
    /// The grant carries no refresh token, so nothing durable was cached.
    ///
    /// The flow asks for one every time (`access_type=offline`,
    /// `prompt=consent`), and a provider may still withhold it — Google does on
    /// a repeat authorization where one was already issued and never revoked.
    /// The access token beside it expires within the hour, which is no use to a
    /// store that has to authorize itself unattended on the next run from the
    /// one long-lived token this flow is supposed to leave behind (spec: SA-6),
    /// so the answer is refused rather than half-cached.
    GrantWithoutRefreshToken,
    /// The grant is not [`DRIVE_FILE_SCOPE`] and nothing else, so nothing was
    /// cached.
    ///
    /// What it is about is not a message but a fact: these are the permissions
    /// the person clicked through on the consent screen, and they travel as the
    /// set the endpoint named so that whoever reports the refusal can say it
    /// their own way. Scopes are not secrets — the tokens that arrived beside
    /// them are, and none of them are carried here.
    GrantNotDriveFileAlone {
        /// What the endpoint said was granted, or `None` where its answer named
        /// no scope at all.
        granted: Option<GrantedScopes>,
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
    /// A call did not become an answer this gateway could use — it did not
    /// land, or what came back is not an answer to it (see [`TransportError`]).
    Transport(TransportError),
    /// The operating system would not supply random bytes for the PKCE
    /// verifier, so no authorization request can be made safely.
    EntropyUnavailable {
        /// What the entropy source reported.
        cause: getrandom::Error,
    },
    /// The Library's app folder could not be created, so the Library has
    /// nowhere on Drive to live (FM-18).
    ///
    /// Named after the step rather than after what went wrong: this happens
    /// before any store exists, and what a caller has to know first is that it
    /// was the folder — not an object in it, and not the grant on its own —
    /// that never came into being.
    AppFolderNotCreated {
        /// The name the folder was to be created under.
        name: String,
        /// What went wrong.
        cause: AppFolderDefect,
    },
    /// Drive would not say what the folder a Library was said to live in is
    /// called, so nothing says which Library that is (FM-18).
    ///
    /// The mirror of [`AppFolderNotCreated`](Self::AppFolderNotCreated) for a
    /// device joining a Library it did not create: the folder is there, and
    /// until its name is read the id names a folder rather than a Library.
    AppFolderUnreadable {
        /// The folder that was asked about.
        folder_id: String,
        /// What went wrong.
        cause: AppFolderDefect,
    },
}

/// What kept an app folder from being created or read.
///
/// As with [`TokenCacheDefect`], the three are one verdict to a caller — there
/// is no folder to work in — and are kept apart so that whichever layer saw the
/// failure has its own answer travel whole.
#[derive(Debug)]
pub enum AppFolderDefect {
    /// The call never succeeded: Drive refused it, or its answer never arrived
    /// whole. It comes classified — whether trying again could help is carried
    /// in it.
    Call(coffret_usecase::Error),
    /// Drive answered, and the answer is not the file resource this build
    /// expects, so nothing in it names a folder.
    Answer(serde_json::Error),
    /// Drive answered with a file resource carrying no name, though the name is
    /// the one field the call asked for.
    Nameless,
}

impl fmt::Display for AppFolderDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call(cause) => write!(f, "{cause}"),
            Self::Answer(cause) => write!(f, "{cause}"),
            Self::Nameless => {
                f.write_str("the answer carries no name, which is what was asked for")
            }
        }
    }
}

impl error::Error for AppFolderDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Call(cause) => Some(cause),
            Self::Answer(cause) => Some(cause),
            Self::Nameless => None,
        }
    }
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
    ///
    /// Every one of those is a fact about the file, which is what makes this a
    /// verdict on the cache. The one thing the format layer refuses that is a
    /// fact about the *caller* — a key derived for another purpose — never
    /// reaches here: [`Error::WrongTokenCacheKey`] is raised before the file is
    /// so much as opened.
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
            Self::WrongTokenCacheKey { path, actual } => write!(
                f,
                "the token cache at {path:?} needs the {} key, and the key given \
                 was derived for {actual}",
                Purpose::TokenCache
            ),
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
            Self::ProviderRefusedAuthorization { refusal } => write!(
                f,
                "authorization did not complete: the request was refused: {refusal}"
            ),
            Self::RedirectWithoutState => f.write_str(
                "authorization did not complete: the redirect did not carry the state \
                 this flow sent",
            ),
            Self::RedirectTimedOut { after } => write!(
                f,
                "authorization did not complete: no redirect arrived within {}s",
                after.as_secs()
            ),
            Self::GrantWithoutRefreshToken => f.write_str(
                "authorization did not complete: the grant carries no refresh token, so \
                 nothing would outlive this run",
            ),
            Self::GrantNotDriveFileAlone {
                granted: Some(granted),
            } => write!(
                f,
                "authorization did not complete: the grant is not {DRIVE_FILE_SCOPE} alone, \
                 but {granted}"
            ),
            Self::GrantNotDriveFileAlone { granted: None } => write!(
                f,
                "authorization did not complete: the token endpoint named no scope, so nothing \
                 says the grant is {DRIVE_FILE_SCOPE} alone"
            ),
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
            Self::Transport(error) => {
                write!(f, "a call did not become a usable answer: {error}")
            }
            Self::EntropyUnavailable { cause } => {
                write!(f, "could not draw random bytes: {cause}")
            }
            Self::AppFolderNotCreated { name, cause } => {
                write!(f, "could not create the app folder {name:?}: {cause}")
            }
            Self::AppFolderUnreadable { folder_id, cause } => {
                write!(f, "could not read the folder {folder_id:?}: {cause}")
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
            Self::AppFolderNotCreated { cause, .. } | Self::AppFolderUnreadable { cause, .. } => {
                Some(cause)
            }
            // Nothing a Rust error reported: what these carry is what a remote
            // said, or a fact this layer put together itself — that nothing was
            // cached, that nothing came back in time, that the key handed over
            // was for another purpose.
            Self::NotAuthorized
            | Self::WrongTokenCacheKey { .. }
            | Self::ProviderRefusedAuthorization { .. }
            | Self::RedirectWithoutState
            | Self::RedirectTimedOut { .. }
            | Self::GrantWithoutRefreshToken
            | Self::GrantNotDriveFileAlone { .. }
            | Self::TokenEndpoint { .. } => None,
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
            | Error::ProviderRefusedAuthorization { .. }
            | Error::RedirectWithoutState
            | Error::RedirectTimedOut { .. }
            | Error::GrantWithoutRefreshToken
            | Error::GrantNotDriveFileAlone { .. }
            | Error::LoopbackRedirect { .. }
            | Error::MalformedRedirect { .. }
            | Error::TokenEndpoint { .. }
            | Error::UnreadableTokenResponse { .. } => Self::Unauthenticated { detail },
            Error::Transport(transport) => transport.into(),
            // Nothing is wrong with the credential or the request: the local
            // machine could not do its part of the work. The port carries an
            // `io::Error`, so the kind the operating system reported is kept —
            // it is what a caller acts on — while the message this layer
            // composed, which names the file, becomes that error's own.
            Error::TokenCache { cause, .. } => Self::Io {
                cause: Arc::new(io::Error::new(cause.kind(), detail)),
            },
            // A cache this build cannot read is this machine's own file too,
            // and it crosses beside the one the operating system refused rather
            // than as a verdict about the credentials. `Io` is the one variant
            // whose rendering in a log is the kind alone; every other one keeps
            // the message, and the message this layer composed names the file.
            // What the person is to do about it is unchanged and is said where
            // they read it, in that message. The file was read and what came
            // out of it is what is wrong, so there is no kind to keep.
            Error::MalformedTokenCache { .. } => Self::Io {
                cause: Arc::new(io::Error::other(detail)),
            },
            // Local failures the operating system was never asked about: the
            // port names every failure of this machine's own part `Io`, and
            // there is no kind to keep. A key derived for another purpose is
            // one of them and not a verdict on the credentials — nothing about
            // the grant has been looked at when this is raised.
            Error::UnencodableTokens { .. }
            | Error::UnsealableTokenCache { .. }
            | Error::WrongTokenCacheKey { .. }
            | Error::EntropyUnavailable { .. } => Self::Io {
                cause: Arc::new(io::Error::other(detail)),
            },
            Error::HttpClient { .. } => Self::Unsupported { detail },
            // The folder this was about is named in the typed error a caller of
            // that operation gets first. A call that failed was already
            // classified by the same code every other Drive call goes through,
            // so it travels as it is rather than being flattened into a message
            // about a folder.
            Error::AppFolderNotCreated { cause, .. } | Error::AppFolderUnreadable { cause, .. } => {
                match cause {
                    AppFolderDefect::Call(cause) => cause,
                    AppFolderDefect::Answer(_) | AppFolderDefect::Nameless => {
                        Self::MalformedResponse { detail }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use coffret_model::Redacted;

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

    // A cache this build cannot read is one of this device's own files, and the
    // message this layer composed about it names that file. Crossing as a local
    // failure is what keeps the name out of the rendering a diagnostic event
    // is built from, while leaving it in the message a person reads.
    #[test]
    fn either_defect_in_a_cache_reaches_the_port_as_a_local_failure() {
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

            let crossed = coffret_usecase::Error::from(error);
            assert!(
                matches!(crossed, coffret_usecase::Error::Io { .. }),
                "{crossed:?}"
            );
            assert!(crossed.to_string().contains("tokens.bin"), "{crossed}");
            assert_eq!(crossed.redacted(), "Io(kind=Other)");
        }
    }

    // The `Io` mapping above is chosen deliberately and explained at length,
    // and what it costs is that the port says "local transfer failed" about a
    // credential store — so the choice is pinned here rather than only
    // described. A reader changing it to `Unauthenticated` would be changing
    // the sentence a person reads, and this is where that shows up.
    #[test]
    fn an_unreadable_cache_reads_as_this_machines_own_failure_at_the_port() {
        let error = Error::MalformedTokenCache {
            path: path(),
            cause: TokenCacheDefect::Sealed(coffret_format::Error::AuthenticationFailed),
        };

        let crossed = coffret_usecase::Error::from(error);
        let rendered = crossed.to_string();
        assert!(
            rendered.starts_with("local transfer failed: "),
            "{rendered}"
        );
        assert!(
            !rendered.contains("Storage rejected the credentials"),
            "the file is this device's own, and Storage was never asked: {rendered}"
        );
        // A file that will not open opens no better on a second read, so no
        // retry loop is to spend an attempt on it.
        assert!(!crossed.is_retryable(), "{rendered}");
    }

    // The mirror of it: a key that was derived for another purpose crosses the
    // same way, because nothing about the credential has been looked at when it
    // is raised — the file was never opened.
    #[test]
    fn a_key_for_another_purpose_reaches_the_port_as_a_local_failure() {
        let error = Error::WrongTokenCacheKey {
            path: path(),
            actual: Purpose::ControlJournal,
        };
        // Nothing a Rust error reported: the fact is which purpose the key
        // carried, and that is what travels.
        assert!(error::Error::source(&error).is_none());

        let crossed = coffret_usecase::Error::from(error);
        assert!(
            matches!(crossed, coffret_usecase::Error::Io { .. }),
            "{crossed:?}"
        );
        assert!(!crossed.is_retryable(), "{crossed}");
        // The file it would have been used on is in the message a person reads
        // and out of the rendering an event is built from (spec: EL-1).
        assert!(crossed.to_string().contains("tokens.bin"), "{crossed}");
        assert_eq!(crossed.redacted(), "Io(kind=Other)");
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

    // A grant wider than the one permission asked for is no credential to work
    // from, whether the endpoint named what it granted or named nothing: the
    // person has to authorize again, which is what the port's
    // `Unauthenticated` says.
    #[test]
    fn a_grant_that_is_not_drive_file_alone_reaches_the_port_as_unauthenticated() {
        let grants = [
            Some(GrantedScopes::parse(&format!(
                "{DRIVE_FILE_SCOPE} https://www.googleapis.com/auth/drive"
            ))),
            None,
        ];
        for granted in grants {
            let error = Error::GrantNotDriveFileAlone { granted };
            // A refusal is worth nothing to its reader without the scope that
            // was expected of the grant.
            assert!(error.to_string().contains(DRIVE_FILE_SCOPE), "{error}");
            assert!(error::Error::source(&error).is_none());
            assert!(matches!(
                coffret_usecase::Error::from(error),
                coffret_usecase::Error::Unauthenticated { .. }
            ));
        }
    }

    // What these four have in common is only where they land: no credential
    // came of the flow, and no retry produces one.
    #[test]
    fn every_way_the_flow_can_end_without_a_grant_reaches_the_port_as_unauthenticated() {
        let endings = [
            (
                Error::ProviderRefusedAuthorization {
                    refusal: "access_denied".to_owned(),
                },
                "access_denied",
            ),
            (Error::RedirectWithoutState, "the state this flow sent"),
            (
                Error::RedirectTimedOut {
                    after: Duration::from_secs(300),
                },
                "within 300s",
            ),
            (Error::GrantWithoutRefreshToken, "no refresh token"),
        ];
        for (error, said) in endings {
            assert!(error.to_string().contains(said), "{error}");
            // What each carries is a fact this layer or the provider stated,
            // never a Rust error it observed.
            assert!(error::Error::source(&error).is_none(), "{error}");

            let crossed = coffret_usecase::Error::from(error);
            assert!(
                matches!(crossed, coffret_usecase::Error::Unauthenticated { .. }),
                "{crossed:?}"
            );
            assert!(!crossed.is_retryable(), "{crossed}");
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

    const FOLDER_NAME: &str = "coffret-0123456789abcdef";

    // A call that failed was classified where Drive's answer was read, and
    // whether trying again could help is what that classification carries. The
    // crossing has to leave it alone rather than name a verdict of its own.
    #[test]
    fn a_call_that_failed_reaches_the_port_as_what_it_was_classified_as() {
        let error = Error::AppFolderNotCreated {
            name: FOLDER_NAME.to_owned(),
            cause: AppFolderDefect::Call(coffret_usecase::Error::RateLimited {
                retry_after: None,
                detail: "the account is calling too often".to_owned(),
            }),
        };

        assert!(error::Error::source(&error).is_some());
        let crossed = coffret_usecase::Error::from(error);
        assert!(
            matches!(crossed, coffret_usecase::Error::RateLimited { .. }),
            "{crossed:?}"
        );
        assert!(crossed.is_retryable());
    }

    #[test]
    fn an_answer_naming_no_folder_reaches_the_port_as_a_malformed_response() {
        let error = Error::AppFolderNotCreated {
            name: FOLDER_NAME.to_owned(),
            cause: AppFolderDefect::Answer(json_error()),
        };

        assert!(error::Error::source(&error).is_some());
        let coffret_usecase::Error::MalformedResponse { detail } =
            coffret_usecase::Error::from(error)
        else {
            panic!("an answer this build cannot read is a malformed response");
        };
        // The port's variant has nowhere to name a folder, so what this layer
        // knew about it has to travel in the message or not at all.
        assert!(detail.contains(FOLDER_NAME), "{detail}");
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
