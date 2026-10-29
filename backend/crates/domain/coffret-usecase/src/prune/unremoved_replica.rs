use coffret_model::ControlObjectName;

use crate::commit::CommitError;

/// A replica of a superseded Keyring generation a prune did not trash
/// (spec: KL-18).
///
/// Storage would not trash it, or one of the objects listed under its name.
/// Nothing is lost by leaving it: no current Container is mapped by its
/// generation alone (spec: KL-7), and the next prune takes it. The reason is
/// kept as the value it was, for the reason [`UnremovedRecord`]'s is.
///
/// No `PartialEq`, for the reason the errors have none.
///
/// [`UnremovedRecord`]: super::UnremovedRecord
#[derive(Debug, Clone)]
pub struct UnremovedReplica {
    /// The name of the replica left in place.
    pub replica: ControlObjectName,
    /// What trashing it failed with.
    pub cause: CommitError,
}
