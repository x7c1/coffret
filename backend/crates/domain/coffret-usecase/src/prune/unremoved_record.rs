use coffret_model::Generation;

use crate::commit::CommitError;

/// An eligible Journal record a prune did not remove (spec: CK-4, CK-6).
///
/// Its header could not be read to say what it is, or Storage would not trash
/// it. Either way nothing is lost: the record is one the Snapshot already
/// covers, so a catch-up starts from the Snapshot and never reads it, and the
/// next prune takes it. The reason is kept as the value it was, for the reason
/// [`UntrashedRemoval`](crate::commit::UntrashedRemoval)'s is: a busy provider,
/// credentials that do not authorize the trash, and an object that is not what
/// its name says are three different next steps.
///
/// No `PartialEq`, for the reason the errors have none.
#[derive(Debug, Clone)]
pub struct UnremovedRecord {
    /// The head generation of the record left in place.
    pub generation: Generation,
    /// What reading its header or trashing it failed with.
    pub cause: CommitError,
}
