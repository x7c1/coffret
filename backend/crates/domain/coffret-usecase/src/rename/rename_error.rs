use std::error;
use std::fmt;

use coffret_model::{Generation, Redacted};

use crate::commit::{CommitError, CommitFailure};
use crate::error::Error;
use crate::index_error::IndexError;
use crate::rename::rename_refusal::RenameRefusal;

/// Result alias for a rename.
pub type RenameResult<T> = std::result::Result<T, RenameError>;

/// Everything a rename can fail with.
///
/// All but the last stop the run before its record exists, so nothing is
/// committed and the Library is as it was (spec: CP-1). The last is the one
/// failure after the commit point, and says so.
///
/// The variants that wrap a layer below say in their own message only which
/// layer the run was in, and leave what that layer reported to the cause they
/// hand on — the reading every flow's error in this crate gives.
///
/// There is deliberately no `PartialEq`: a caller decides from the variant and
/// the fields it names, never by comparing two errors.
#[derive(Debug)]
pub enum RenameError {
    /// Storage failed, or answered something the run cannot go on from.
    Storage(Error),
    /// The Index could not be read or written before the commit.
    Index(IndexError),
    /// The run entered the commit flow and did not come through it.
    ///
    /// Including the catch-up before the plan, which is the commit flow's
    /// routine (spec: CK-9), and a rename another device's commit meanwhile
    /// leaves no way to carry over, which is
    /// [`CommitError::RebaseConflict`] (spec: EP-7).
    Commit(CommitFailure),
    /// The rename was refused before anything was written.
    Refused(RenameRefusal),
    /// The rename committed, and this device's catalog could not say which of
    /// its files wait to follow it.
    ///
    /// The Library holds the renames: the files stay where they were, their
    /// records waiting, and the next sync or fetch relocates them
    /// (spec: EP-10).
    NotRelocated {
        /// The head the rename committed.
        generation: Generation,
        /// What the catalog reported.
        cause: IndexError,
    },
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(_) => f.write_str("the rename did not get what it asked of Storage"),
            Self::Index(_) => f.write_str("the rename could not read the Index"),
            Self::Commit(_) => f.write_str("the rename did not come through the commit flow"),
            Self::Refused(refusal) => write!(f, "the rename was refused: {refusal}"),
            Self::NotRelocated { generation, .. } => write!(
                f,
                "the rename committed at head {generation}, and this device's files of the \
                 renamed Entries were not relocated; the next sync or fetch relocates them"
            ),
        }
    }
}

impl error::Error for RenameError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Index(error) => Some(error),
            Self::Commit(error) => Some(error),
            // The refusal is said in this message already, so it is not handed
            // on to be said a second time.
            Self::Refused(_) => None,
            Self::NotRelocated { cause, .. } => Some(cause),
        }
    }
}

impl Redacted for RenameError {
    /// No Entry Path: the shape of the failure kept whole.
    fn redacted(&self) -> String {
        match self {
            Self::Storage(error) => format!("Rename::Storage: {}", error.redacted()),
            Self::Index(error) => format!("Rename::Index: {}", error.redacted()),
            Self::Commit(error) => format!("Rename::Commit: {}", error.redacted()),
            Self::Refused(refusal) => format!("Rename::Refused: {}", refusal.redacted()),
            Self::NotRelocated { generation, cause } => format!(
                "Rename::NotRelocated(head={generation}): {}",
                cause.redacted()
            ),
        }
    }
}

impl From<Error> for RenameError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for RenameError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<CommitError> for RenameError {
    fn from(error: CommitError) -> Self {
        Self::Commit(error.into())
    }
}

impl From<CommitFailure> for RenameError {
    fn from(failure: CommitFailure) -> Self {
        Self::Commit(failure)
    }
}

impl From<RenameRefusal> for RenameError {
    fn from(refusal: RenameRefusal) -> Self {
        Self::Refused(refusal)
    }
}
