use coffret_model::ObjectRef;

use crate::provider_hash::ProviderHash;

/// What a listing reports about one object in Storage.
///
/// Everything here is what the provider knows without opening the object:
/// coffret hands Storage only ciphertext, so a listing is a directory of names
/// — opaque for Containers, recognizable for the control objects recovery has
/// to discover by name before any Index exists (spec: FM-12) — and nothing in
/// it is trusted for anything but locating objects. The [`ObjectRef`] is what
/// later calls act on — on a store that names objects by an identifier of its
/// own, the name and the reference are different strings.
///
/// It carries what a caller reads and nothing more. A provider states other
/// things about a listed object — a stored size, a modification time of its own
/// bookkeeping — and a field nobody reads is a field an adapter fills in for a
/// provider that left it out, which is how "the provider said zero" and "the
/// provider said nothing" become the same value. When a caller comes to need
/// one of those, it comes back as an `Option` stating what that provider means
/// by it, so an answer and an absence stay apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    /// What later calls name this object by.
    pub object_ref: ObjectRef,
    /// The name the object was stored under.
    pub name: String,
    /// The digest the provider reports for the stored bytes, on the terms
    /// [`ProviderHash`] states, or `None` where it reports none.
    ///
    /// A provider that names no digest has answered rather than answered
    /// badly, which is why this absence is an `Option` here and not a refusal
    /// at the adapter.
    pub hash: Option<ProviderHash>,
}
