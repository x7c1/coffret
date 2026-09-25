//! The model's own refusal, carried as [`Error::Model`].

use super::Error;

impl From<coffret_model::Error> for Error {
    fn from(error: coffret_model::Error) -> Self {
        Self::Model(error)
    }
}
