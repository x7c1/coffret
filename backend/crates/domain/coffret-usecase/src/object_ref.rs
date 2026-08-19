use std::fmt;

/// A handle to one object in Storage, in whatever form its provider names it.
///
/// A store that keys objects by name puts the name here; a store that mints an
/// identifier of its own — a Google Drive file ID — puts that. Callers never
/// parse it: they take it from a [`put`](crate::ObjectStore::put) or a
/// [`list`](crate::ObjectStore::list) and hand it back.
///
/// The handle survives [`trash`](crate::ObjectStore::trash): a trashed object is
/// still the same object, so the reference that named it before names it after,
/// and [`purge`](crate::ObjectStore::purge) accepts it either way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef(String);

impl ObjectRef {
    /// Takes the provider's own handle for an object.
    pub fn new(handle: impl Into<String>) -> Self {
        Self(handle.into())
    }

    /// The handle as the provider spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
