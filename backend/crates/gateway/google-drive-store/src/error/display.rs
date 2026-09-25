//! The line each [`Error`] variant is read out as.

use std::fmt;

use coffret_format::Purpose;

use crate::oauth::DRIVE_FILE_SCOPE;

use super::Error;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Every arm below that has a `cause` says which step of this
            // layer's work refused and what it was working on, and stops
            // there. What the library, the operating system, the format layer
            // or the layer under this one answered is the value `source` hands
            // on, which a caller walking the chain prints underneath —
            // rendering it in here as well would spell one refusal twice over.
            Self::HttpClient { .. } => f.write_str("could not build an HTTP client"),
            Self::TokenCache { path, .. } => {
                write!(f, "could not use the token cache at {path:?}")
            }
            Self::MalformedTokenCache { path, .. } => {
                write!(f, "the token cache at {path:?} is unreadable")
            }
            Self::WrongTokenCacheKey { path, actual } => write!(
                f,
                "the token cache at {path:?} needs the {} key, and the key given \
                 was derived for {actual}",
                Purpose::TokenCache
            ),
            Self::UnencodableTokens { path, .. } => {
                write!(f, "could not encode the tokens for the cache at {path:?}")
            }
            Self::UnsealableTokenCache { path, .. } => {
                write!(f, "could not seal the token cache at {path:?}")
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
            Self::LoopbackRedirect { step, .. } => {
                write!(f, "authorization did not complete: {step}")
            }
            Self::MalformedRedirect { target, .. } => {
                write!(
                    f,
                    "authorization did not complete: the redirect target {target:?} is not a URL"
                )
            }
            Self::TokenEndpoint { status, detail } => {
                write!(f, "the token endpoint answered {status}: {detail}")
            }
            Self::CodeExchangeWithoutSecret { status, detail } => write!(
                f,
                "the token endpoint answered {status} to a code exchange made without a \
                 client secret, which a client registered with one cannot be authorized \
                 without: {detail}"
            ),
            Self::UnreadableTokenResponse { status, .. } => {
                write!(
                    f,
                    "the token endpoint answered {status}: unreadable token response"
                )
            }
            Self::Transport(_) => f.write_str("a call did not become a usable answer"),
            Self::EntropyUnavailable { .. } => f.write_str("could not draw random bytes"),
            Self::AppFolderNotCreated { name, .. } => {
                write!(f, "could not create the app folder {name:?}")
            }
            Self::AppFolderUnreadable { folder_id, .. } => {
                write!(f, "could not read the folder {folder_id:?}")
            }
            Self::LibraryObjectUnreadable {
                folder_id, name, ..
            } => write!(
                f,
                "could not tell whether the folder {folder_id:?} holds {name:?}"
            ),
        }
    }
}
