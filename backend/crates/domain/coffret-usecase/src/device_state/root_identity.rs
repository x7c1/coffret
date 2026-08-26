use std::fmt;

/// What the filesystem under one mapped local root was when a scan last saw it
/// (spec: EP-12).
///
/// A mapped root that is not there, and one that is there but empty because the
/// disk it stood on is unmounted, are the two shapes an unavailable root takes.
/// The first is answered by asking whether the directory exists; the second
/// needs something to compare against, and this is it: a scan that finds the
/// root stamps the mapping with the filesystem it stood on, so a later scan
/// finding an empty root on a *different* filesystem knows it is looking at what
/// an unmount left behind rather than at a folder the user emptied.
///
/// What a platform can say about "the filesystem this folder stands on" differs
/// between platforms, so the value is opaque: it is compared only against
/// another value this same device recorded, and it is never parsed, split, or
/// ordered against meaning. The one function that knows what is inside it is the
/// one that reads it off a directory's metadata, and the spelling carries a tag
/// for the form it came from so a value one platform recorded can never compare
/// equal to a value another platform's form happened to spell the same way —
/// which matters because that comparison is what decides whether deletion
/// inference runs at all.
///
/// It never leaves the device — no Journal record or Snapshot carries it
/// (spec: CK-7).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootIdentity(String);

impl RootIdentity {
    /// Takes the device's own spelling of a filesystem's identity.
    pub fn new(identity: impl Into<String>) -> Self {
        Self(identity.into())
    }

    /// The identity as this device spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RootIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
