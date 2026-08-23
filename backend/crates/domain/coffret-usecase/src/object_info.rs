use coffret_model::{Mtime, ObjectRef};

use crate::provider_hash::ProviderHash;

/// What a listing reports about one object in Storage.
///
/// Everything here is what the provider knows without opening the object:
/// coffret hands Storage only ciphertext, so a listing is a directory of names
/// and sizes — opaque for Containers, recognizable for the control objects
/// recovery has to discover by name before any Index exists (spec: FM-12) — and
/// nothing in it is trusted for anything but locating objects. The
/// [`ObjectRef`] is what later calls act on — on a store that names objects by
/// an identifier of its own, the name and the reference are different strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    /// What later calls name this object by.
    pub object_ref: ObjectRef,
    /// The name the object was stored under.
    pub name: String,
    /// The stored size in bytes.
    pub size: u64,
    /// When the provider last saw the object change.
    ///
    /// Storage's own bookkeeping about the ciphertext it holds, not the
    /// modification time of any Entry inside it, which travels encrypted.
    pub mtime: Mtime,
    /// The provider's own digest of the stored bytes, when it reports one.
    pub hash: Option<ProviderHash>,
}
