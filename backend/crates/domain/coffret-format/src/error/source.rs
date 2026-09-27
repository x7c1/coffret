//! Which failure each [`Error`] variant carries under its own line, if any.

use std::error;

use super::Error;

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::EntropyUnavailable { cause } => Some(cause),
            Self::InvalidArgon2Params { cause } | Self::PassphraseDerivationFailed { cause } => {
                Some(cause)
            }
            Self::MalformedMeta { detail }
            | Self::MalformedControlPayload { detail }
            | Self::MalformedJournalRecord { detail }
            | Self::MalformedIndexSnapshot { detail }
            | Self::MalformedKeyringReplica { detail } => detail
                .cause()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            _ => None,
        }
    }
}
