use std::path::PathBuf;
use std::sync::Arc;

use coffret_device::{EntryPath, EntryRef, KeptAside};

/// One file an add wrote on this device, as it is kept for the add's undo
/// (spec: LA-36).
#[derive(Clone, Debug)]
pub(crate) struct Written {
    /// Where it stands in the Library once a run carries it in.
    pub(crate) path: EntryPath,
    /// Where it stands on this device.
    pub(crate) local: PathBuf,
    /// The file this add left there, as the filesystem names it — device and
    /// inode — so that an undo removes that file and not one put in its place
    /// since; `None` where it could not be read.
    pub(crate) identity: Option<(u64, u64)>,
    /// What stood at `local` before this add wrote there.
    pub(crate) replaced: Replaced,
    /// The Entry the Library held at the path before this add, so that an
    /// undo can tell a run that committed this add's file — the Entry there is
    /// another one now — from one that did not.
    pub(crate) entry_before: Option<EntryRef>,
}

/// What a file an add wrote took the place of on this device (spec: LA-36).
#[derive(Clone, Debug)]
pub(crate) enum Replaced {
    /// Nothing stood there: an undo removes the file.
    Nothing,
    /// A file stood there, kept aside until the add ends: an undo puts it back.
    /// Shared, because the add is read out of the registry by copy; the last
    /// copy to go lets the kept file go (spec: OC-8).
    Kept(Arc<KeptAside>),
    /// A file stood there that this folder's filesystem would not keep aside,
    /// so nothing could put it back: the add is not stopped at all
    /// ([`Adds::ask_stop`](super::Adds::ask_stop)).
    Unkept,
}
