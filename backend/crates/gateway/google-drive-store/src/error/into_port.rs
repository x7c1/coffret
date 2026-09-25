//! What an [`Error`] becomes when it reaches the port, and the walk of its chain
//! that a local failure's message is built from.

use std::error;
use std::io;
use std::sync::Arc;

use coffret_usecase::GatewayFailure;

use crate::http::TransportError;

use super::{AppFolderDefect, Error, TokenCacheDefect, TokenResponseDefect};

/// Whether a link in a chain is one this workspace wrote.
///
/// Listed by type rather than decided by a trait, because being this
/// workspace's own is not something a foreign error could be asked. Every type
/// that can stand in a chain rooted at this crate's [`Error`] is here: this
/// crate's own vocabularies, and the three layers below whose errors reach
/// them. One left out is read as foreign, which stops the walk a link early
/// rather than carrying something it should not.
fn is_this_workspaces_own(error: &(dyn error::Error + 'static)) -> bool {
    error.is::<Error>()
        || error.is::<AppFolderDefect>()
        || error.is::<TokenCacheDefect>()
        || error.is::<TokenResponseDefect>()
        || error.is::<TransportError>()
        || error.is::<coffret_format::Error>()
        || error.is::<coffret_usecase::Error>()
        || error.is::<coffret_model::Error>()
}

/// `error`'s chain down to the first error this workspace did not write, that
/// one's own sentence included, joined the way the binaries print a chain.
///
/// The name says where it stops, because stopping is the whole of what
/// distinguishes it from the walk the examples keep (`every_link` in
/// `examples/support/mod.rs`), which follows a chain all the way down.
///
/// It is the message of the `io::Error` a local failure crosses the port in,
/// and only that. The port's `Io` carries what the operating system reported
/// rather than this crate's error, so the one place this layer's account of a
/// local failure can travel is that error's own message: `coffret-cli` and
/// `coffret-server` end on `eprintln!("{error:#}")`, which spells a chain as
/// its links separated by `": "`, so a message built this way reads to a person
/// exactly as the chain would have. Every other failure crosses with this
/// crate's error itself as the port's `source`, and a chain walked from there
/// needs nothing rendered in advance.
///
/// It stops at the first foreign link because a foreign `Display` renders
/// itself and not its sources, which is where the flattening this crate's
/// wrappers used to do stopped anyway.
fn chain_to_the_workspace_edge(error: &(dyn error::Error + 'static)) -> String {
    let mut rendered = error.to_string();
    let mut below = error.source();
    while let Some(link) = below {
        rendered.push_str(": ");
        rendered.push_str(&link.to_string());
        if !is_this_workspaces_own(link) {
            break;
        }
        below = link.source();
    }
    rendered
}

/// What a failure about a Library's app folder says, read before the error
/// holding it is handed across whole.
enum FolderVerdict {
    /// A call that failed, already classified by the same code every other
    /// Drive call goes through.
    Classified(coffret_usecase::Error),
    /// Drive answered, and the answer names no folder this build can read.
    Unreadable,
    /// Drive answered every page and never said the listing was over.
    Unending {
        /// How many pages were read before the walk gave up.
        pages: usize,
    },
}

impl FolderVerdict {
    fn of(defect: &AppFolderDefect) -> Self {
        match defect {
            AppFolderDefect::Call(cause) => Self::Classified(cause.clone()),
            AppFolderDefect::Answer(_) | AppFolderDefect::Nameless => Self::Unreadable,
            AppFolderDefect::UnendingListing { pages } => Self::Unending { pages: *pages },
        }
    }

    /// How a failure about a Library's app folder reads in the port's
    /// vocabulary.
    ///
    /// The folder this was about is named in the typed error a caller of that
    /// operation gets first. A call that failed travels as it was classified
    /// rather than being flattened into a message about a folder. A listing
    /// that never ended is the port's own [`ListingPastCap`], because every
    /// page of it was answered and read: calling it an answer this build cannot
    /// read would send somebody looking for a response that does not exist.
    ///
    /// [`ListingPastCap`]: coffret_usecase::Error::ListingPastCap
    fn into_port(self, error: Error, detail: String) -> coffret_usecase::Error {
        match self {
            Self::Classified(cause) => cause,
            Self::Unreadable => coffret_usecase::Error::MalformedResponse {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            Self::Unending { pages } => coffret_usecase::Error::ListingPastCap {
                pages,
                source: Some(GatewayFailure::new(error)),
            },
        }
    }
}

impl From<Error> for coffret_usecase::Error {
    fn from(error: Error) -> Self {
        // The error itself crosses as `source`, because
        // [`coffret_usecase::Error`] lives in the domain and cannot name this
        // type, and what is not handed over whole is gone. The port's line for
        // the kind of failure is printed above it and the chain under it, so
        // `detail` carries only this error's own top line — for a caller that
        // reads the field without walking — and rendering the chain into it as
        // well would say every sentence below twice.
        //
        // A local failure crosses as `Io`, which carries the operating
        // system's error rather than this one, so there the message has to
        // hold the whole chain or lose it (`chain_to_the_workspace_edge`).
        //
        // The arms match in place and bind by reference, so that the error is
        // still whole to hand over once its variant has been read.
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
            | Error::MalformedRedirect { .. }
            | Error::TokenEndpoint { .. }
            | Error::CodeExchangeWithoutSecret { .. } => Self::Unauthenticated {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            // A token response whose body broke off in transit says nothing
            // about the grant: the endpoint was answering, and the connection
            // under it gave out. Folding it into `Unauthenticated` told the
            // person to renew a grant that was never in question, and told the
            // retry loop to give up on a call that the next attempt may well
            // complete — the same call failing before its body began already
            // crosses as a transport failure, through `Transport` below. So
            // whether it is worth another attempt is read off the verdict the
            // body's own drain reached, which is already in the port's
            // vocabulary: a transfer that broke is `Transport`, and one refused
            // for what it was — longer than any token response can be — is an
            // answer this build cannot read. A drain into memory has no local
            // end of its own, so an `Io` out of it is the connection's reader
            // giving out, and is a transfer that broke like the rest.
            Error::UnreadableTokenResponse {
                cause: TokenResponseDefect::Body(ref drained),
                ..
            } if drained.is_retryable() || matches!(drained, coffret_usecase::Error::Io { .. }) => {
                Self::Transport {
                    detail,
                    source: Some(GatewayFailure::new(error)),
                }
            }
            // A token response that arrived whole and is not the document this
            // build expects is the endpoint saying yes in a shape nobody here
            // can read — a mint is the only answer this is raised for — which
            // is not a verdict on the grant either. It is an answer this build
            // cannot read, and like every such answer it is not retried.
            Error::UnreadableTokenResponse { .. } => Self::MalformedResponse {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            Error::Transport(transport) => transport.into(),
            // Nothing is wrong with the credential or the request: the local
            // machine could not do its part of the work. The port carries an
            // `io::Error`, so the kind the operating system reported is kept —
            // it is what a caller acts on — while the message this layer
            // composed, which names the file, becomes that error's own.
            //
            // The loopback the browser comes back to is this machine's own work
            // in exactly that way. It crossed as `Unauthenticated` once, which
            // reads as Storage having rejected the credentials when Storage was
            // never asked anything: a port already taken is the operating
            // system's answer, and the kind it gave is what says so.
            Error::TokenCache { ref cause, .. } | Error::LoopbackRedirect { ref cause, .. } => {
                Self::Io {
                    cause: Arc::new(io::Error::new(
                        cause.kind(),
                        chain_to_the_workspace_edge(&error),
                    )),
                }
            }
            // A cache this build cannot read is this machine's own file too,
            // and it crosses beside the one the operating system refused rather
            // than as a verdict about the credentials. `Io` is the variant whose
            // rendering in a log is the kind alone, and the message this layer
            // composed names the file. What the person is to do about it is
            // unchanged and is said where they read it, in that message. The
            // file was read and what came out of it is what is wrong, so there
            // is no kind to keep.
            Error::MalformedTokenCache { .. } => Self::Io {
                cause: Arc::new(io::Error::other(chain_to_the_workspace_edge(&error))),
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
                cause: Arc::new(io::Error::other(chain_to_the_workspace_edge(&error))),
            },
            Error::HttpClient { .. } => Self::Unsupported {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            Error::AppFolderNotCreated { ref cause, .. }
            | Error::AppFolderUnreadable { ref cause, .. } => {
                FolderVerdict::of(cause).into_port(error, detail)
            }
            // The defect is boxed in this one variant and in no other, so it is
            // read through the box the same way.
            Error::LibraryObjectUnreadable { ref cause, .. } => {
                FolderVerdict::of(cause).into_port(error, detail)
            }
        }
    }
}
