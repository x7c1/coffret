use crate::error::Error;

/// What became of the object an abandoned batch left on Storage, if it left
/// one (spec: OC-2, OC-3).
///
/// Three answers rather than a yes or a no, because two of them are opposite
/// states that a `false` would read alike. A spool that never left the device
/// put nothing on Storage, so there was nothing to remove; an object Storage
/// would not move to the trash is still there, named by no current state, for
/// orphan cleanup to find (spec: OC-1, OC-4). Only the refused trash leaves
/// anything behind, and a caller that is told "disposed of" for both tells a
/// person the Library's Storage holds less than it does.
///
/// There is deliberately no `PartialEq`, for the reason
/// [`UntrashedRemoval`](crate::commit::UntrashedRemoval) has none: a caller
/// decides from the variant, and from the variant of the cause where there is
/// one.
#[derive(Debug, Clone)]
pub enum Disposal {
    /// The earlier run never uploaded the Container, so nothing of it was on
    /// Storage to remove.
    NeverUploaded,
    /// Its object was moved to the provider's trash.
    Trashed,
    /// Storage refused to move its object to the trash, so it is still there.
    ///
    /// The row that was the provenance for it goes regardless, so what is left
    /// is an object no current state names, which is orphan cleanup's to find
    /// and a person's to decide on (spec: OC-1, OC-4).
    LeftInStorage {
        /// What Storage answered the trash with.
        cause: Error,
    },
}
