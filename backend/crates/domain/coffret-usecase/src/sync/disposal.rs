use crate::error::Error;

/// What became of the object an abandoned batch left on Storage, if it left
/// one (spec: OC-2, OC-3).
///
/// Three answers rather than a yes or a no, because two of them are opposite
/// states that a `false` would read alike. A Container that never reached
/// Storage left nothing there to remove; an object Storage would not trash is
/// still there, and its row with it, for a later run to retry (spec: OC-2,
/// OC-3). Only the refused trash leaves anything behind, and a caller that is
/// told "disposed of" for both tells a person the Library's Storage holds less
/// than it does.
///
/// There is deliberately no `PartialEq`, for the reason
/// [`UntrashedRemoval`](crate::commit::UntrashedRemoval) has none: a caller
/// decides from the variant, and from the variant of the cause where there is
/// one.
#[derive(Debug, Clone)]
pub enum Disposal {
    /// Nothing of the Container was on Storage to remove: the earlier run never
    /// put it, or its put never landed.
    NeverUploaded,
    /// Its object was trashed on Storage, whether the row recorded the object or
    /// only its name did.
    Trashed,
    /// Storage refused to trash its object, so it is still there.
    ///
    /// The row that was the provenance for it is retained, because it is the
    /// proof that makes the next run's retry safe (spec: OC-2, OC-3).
    LeftInStorage {
        /// What Storage answered the trash with.
        cause: Error,
    },
}
