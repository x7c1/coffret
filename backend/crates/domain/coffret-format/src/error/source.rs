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
            _ => None,
        }
    }
}
