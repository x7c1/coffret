//! What the cases of this module and its siblings build their errors out of.

use crate::commit::CommitError;
use crate::error::Error;

/// What a provider that failed on its own side leaves behind.
pub(super) fn provider_fault() -> CommitError {
    CommitError::Storage(Error::ServiceUnavailable {
        status: 503,
        detail: "backendError".to_owned(),
        source: None,
    })
}

/// What an object that arrived and would not open leaves behind.
pub(super) fn unopenable() -> CommitError {
    CommitError::Format(coffret_format::Error::AuthenticationFailed)
}
