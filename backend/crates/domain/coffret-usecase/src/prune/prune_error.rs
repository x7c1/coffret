use std::error;
use std::fmt;

use coffret_model::Redacted;

use crate::commit::CommitError;
use crate::prune::prune_refusal::PruneRefusal;

/// Result alias for a prune.
pub type PruneResult<T> = std::result::Result<T, PruneError>;

/// Everything a prune can fail with.
///
/// Both stop the run before it removes anything. A record it could not remove
/// once it had started is not here: the records before it stay removed, so it
/// is reported in the [`PruneOutcome`](super::PruneOutcome) instead.
///
/// No `PartialEq`, for the reason every flow's error in this crate has none.
#[derive(Debug)]
pub enum PruneError {
    /// The run did not come through the commit flow's own steps: the catch-up,
    /// reading the head, writing or reading the Snapshot, or reading the
    /// Keyring (spec: CK-9, CK-10, CK-11, KL-1).
    Commit(CommitError),
    /// The gate refused, and nothing was removed (spec: CK-5).
    ///
    /// Boxed because a refusal carries two Keyring tuples, and an error that
    /// large would make every `Result` of the flow carry them too.
    Refused(Box<PruneRefusal>),
}

impl fmt::Display for PruneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Commit(_) => f.write_str("the prune did not come through the commit flow"),
            Self::Refused(refusal) => write!(f, "the prune was refused: {refusal}"),
        }
    }
}

impl error::Error for PruneError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Commit(error) => Some(error),
            // Said in this message already, so not handed on to be said twice.
            Self::Refused(_) => None,
        }
    }
}

impl Redacted for PruneError {
    fn redacted(&self) -> String {
        match self {
            Self::Commit(error) => format!("Prune::Commit: {}", error.redacted()),
            Self::Refused(refusal) => format!("Prune::Refused: {}", refusal.redacted()),
        }
    }
}

impl From<CommitError> for PruneError {
    fn from(error: CommitError) -> Self {
        Self::Commit(error)
    }
}

impl From<PruneRefusal> for PruneError {
    fn from(refusal: PruneRefusal) -> Self {
        Self::Refused(Box::new(refusal))
    }
}
