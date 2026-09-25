//! Which failure each [`Error`] variant carries under its own line, if any.

use std::error;

use super::Error;

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
            Self::LibraryObjectUnreadable { cause, .. } => Some(cause.as_ref()),
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
            | Self::TokenEndpoint { .. }
            | Self::CodeExchangeWithoutSecret { .. } => None,
        }
    }
}
