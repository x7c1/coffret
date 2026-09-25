//! A transport failure, carried as [`Error::Transport`].

use crate::http::TransportError;

use super::Error;

impl From<TransportError> for Error {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}
