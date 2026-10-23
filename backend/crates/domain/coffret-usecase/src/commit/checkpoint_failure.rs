use std::fmt;

use coffret_model::{ControlObjectName, Redacted};

use crate::commit::commit_error::{CommitError, ControlObjectFault};
use crate::error::Error;

/// Why a checkpoint a commit was due to write was not written (spec: CK-8).
///
/// A value of its own rather than the [`CommitError`] the write failed with,
/// because what a failed checkpoint leaves is a finding a run reports after its
/// commit stood, and the ways a checkpoint write fails that a caller could
/// decide on are named here: a Storage refusal, a format fault, a slot something
/// else stands at. Each keeps its cause as the value it was, so a caller can
/// match on it rather than on a sentence; any other failure is kept as the
/// [`CommitError`] itself.
///
/// What it renders is the redacted form throughout, because what a run reports
/// is read on a terminal and in an explorer alike, and a checkpoint's
/// bookkeeping — a Storage status, a control-object name — is all a person
/// needs of it (spec: EL-1).
#[derive(Debug, Clone)]
pub enum CheckpointFailure {
    /// Storage refused the Snapshot or could not be reached, after the retries
    /// the commit's policy allows; or the snapshot slot kept refusing a create
    /// while holding nothing.
    Storage(Error),
    /// The Snapshot could not be encoded, or what stood at its slot could not
    /// be opened by this build.
    Format(coffret_format::Error),
    /// Something other than a Snapshot of this head stands at its snapshot slot.
    ///
    /// Reported rather than overwritten or written under another name: a
    /// second name for one head would leave readers two checkpoints to choose
    /// between (spec: CK-11).
    SlotTaken {
        /// The name the slot was read under.
        object: ControlObjectName,
        /// What about the object there did not hold.
        fault: ControlObjectFault,
    },
    /// Any other way the write failed.
    Other {
        /// What the write failed with.
        cause: CommitError,
    },
}

impl CheckpointFailure {
    /// What a checkpoint write that failed with `error` failed at.
    ///
    /// Not a `From`, because the conversion holds only for the checkpoint
    /// write: there the one control object read is what stands at the snapshot
    /// slot, so a corrupt one is that slot taken. Elsewhere in a commit the
    /// same error names another object entirely.
    pub(super) fn of_checkpoint_write(error: CommitError) -> Self {
        match error {
            CommitError::Storage(error) => Self::Storage(error),
            CommitError::Format(error) => Self::Format(error),
            CommitError::CorruptControlObject { object, fault } => {
                Self::SlotTaken { object, fault }
            }
            cause => Self::Other { cause },
        }
    }
}

impl Redacted for CheckpointFailure {
    /// The Storage refusal, the format fault, or the slot's object name, each
    /// in its own redacted form.
    fn redacted(&self) -> String {
        match self {
            Self::Storage(error) => format!("Checkpoint::Storage: {}", error.redacted()),
            Self::Format(error) => format!("Checkpoint::Format: {}", error.redacted()),
            Self::SlotTaken { object, fault } => {
                format!(
                    "Checkpoint::SlotTaken(object={object}): {}",
                    fault.redacted()
                )
            }
            Self::Other { cause } => format!("Checkpoint::Other: {}", cause.redacted()),
        }
    }
}

/// The redacted form, so that a finding built from this says nothing the log
/// would not (spec: EL-1).
impl fmt::Display for CheckpointFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.redacted())
    }
}

#[cfg(test)]
mod tests {
    use coffret_model::Generation;

    use super::*;
    use crate::generations::generation;

    // A Storage refusal keeps its value, so a caller can match on it, and says
    // only its redacted form: the provider's own detail stays out (spec: EL-1).
    #[test]
    fn a_storage_refusal_is_kept_as_a_value_and_said_redacted() {
        let failure =
            CheckpointFailure::of_checkpoint_write(CommitError::Storage(Error::Rejected {
                status: 503,
                detail: "the provider is unavailable".to_owned(),
                source: None,
            }));

        assert!(
            matches!(
                failure,
                CheckpointFailure::Storage(Error::Rejected { status: 503, .. })
            ),
            "{failure:?}",
        );
        let said = failure.to_string();
        assert_eq!(said, "Checkpoint::Storage: Storage::Rejected(status=503)");
        assert!(!said.contains("the provider is unavailable"), "{said}");
    }

    // What stands at the snapshot slot keeps its fault as a value, so a caller
    // can tell a Snapshot of another head from bytes this build cannot open.
    #[test]
    fn a_taken_slot_keeps_its_fault_as_a_value() {
        let object = ControlObjectName::index_snapshot(Generation::FIRST);
        let failure = CheckpointFailure::of_checkpoint_write(CommitError::CorruptControlObject {
            object: object.clone(),
            fault: ControlObjectFault::GenerationMismatch {
                found: generation(1),
            },
        });

        assert!(
            matches!(
                &failure,
                CheckpointFailure::SlotTaken { fault, .. }
                    if matches!(fault, ControlObjectFault::GenerationMismatch { .. })
            ),
            "{failure:?}",
        );
        assert!(
            failure
                .to_string()
                .starts_with(&format!("Checkpoint::SlotTaken(object={object}): ")),
            "{failure}",
        );
    }

    // Any other cause is kept as the commit error it was, so a caller can still
    // match on it, and is said in its redacted form.
    #[test]
    fn any_other_cause_is_kept_as_the_commit_error_it_was() {
        let failure = CheckpointFailure::of_checkpoint_write(CommitError::FirstHeadUnlisted);

        assert!(
            matches!(
                &failure,
                CheckpointFailure::Other {
                    cause: CommitError::FirstHeadUnlisted
                }
            ),
            "{failure:?}",
        );
        assert_eq!(
            failure.to_string(),
            "Checkpoint::Other: Commit::FirstHeadUnlisted"
        );
    }
}
