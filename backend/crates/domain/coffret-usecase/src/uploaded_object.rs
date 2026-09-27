use coffret_model::ObjectRef;

use crate::provider_hash::ProviderHash;

/// What Storage answers an upload with: the handle of the object it stored,
/// and its digest of the bytes it stored where it names one.
///
/// The digest is what lets a caller confirm the object arrived whole from the
/// answer to the write itself, rather than by asking the listing afterwards —
/// which would be a walk over the whole Library, and on a store whose names are
/// not unique, a lookup that could answer about another object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedObject {
    /// What later calls name this object by.
    pub object_ref: ObjectRef,
    /// The digest the provider reports for the stored bytes, on the terms
    /// [`ProviderHash`] states, or `None` where its answer names none.
    ///
    /// An adapter whose provider always names one refuses an answer that does
    /// not, so `None` here is a provider that has no digest to give rather than
    /// one that left it out.
    pub hash: Option<ProviderHash>,
}
