use std::path::PathBuf;

use crate::below_root_error::BelowRootError;
use crate::kept_file::Aside;
use crate::scratch_file::ScratchFile;

/// The folder one file belongs in, held open, reached without passing through
/// anything but a real directory of the mapped root.
///
/// What [`Destinations::reach`](crate::Destinations::reach) hands back, and it
/// is a handle on a folder rather than a path deliberately. Every call below is
/// made *relative to it* — the scratch, the rename that publishes it, the
/// removal that cleans it up — so no answer can go stale between the descent and
/// the write. A path rebuilt from the root and handed back to the operating
/// system would ask the question again, and a name that became a symbolic link
/// in between would be followed on that second asking (spec: EP-4, EP-11).
///
/// It also holds the file's own name, which is the last of the components the
/// descent was given: [`publish`](crate::FlushedFile::publish) is what puts a
/// file at that name, and nothing else here needs to say it.
///
/// It is not [`Sync`]: one of these belongs to the step placing one file, and
/// nothing shares it.
pub trait Destination: Send {
    /// Makes a new file in the folder and hands it back open for writing.
    ///
    /// Exclusive: a name that already exists is a refusal rather than a file two
    /// writers share, and a symbolic link that took the name first is refused
    /// instead of followed. Callers give it a scratch name
    /// ([`scratch`](crate::scratch)), which nothing else in the folder is using.
    ///
    /// Synchronous, because it is one call against a folder that is already
    /// open — the same reason [`remove`](Self::remove) is. The bytes that follow
    /// are what may take a while, and [`ScratchFile`] is where they go.
    ///
    /// # Errors
    ///
    /// [`BelowRootError::Io`] carrying
    /// [`Creating`](crate::LocalOperation::Creating) where the file could not be
    /// made — a name anything at all already stands at included, since what an
    /// exclusive create refuses it refuses without looking at what is there.
    fn create(&self, scratch_name: &str) -> Result<Box<dyn ScratchFile>, BelowRootError>;

    /// Removes one of the folder's own files.
    ///
    /// One that is already gone is the outcome this wanted, so a cleanup racing
    /// the failure it is cleaning up after still succeeds — the same tolerance
    /// [`Spool::discard`](crate::Spool::discard) has, and for the same reason
    /// (spec: OC-8, EP-11).
    ///
    /// Synchronous, and that is part of the contract rather than an
    /// implementation's choice: a value that drops without having published its
    /// file removes it in `Drop`, and a `Drop` cannot await. It is one call
    /// against a folder that is already open.
    fn remove(&self, name: &str) -> Result<(), BelowRootError>;

    /// Keeps the file standing at the file's own name aside under
    /// `kept_name`, a scratch name ([`scratch`](crate::scratch)), by a second
    /// link to it in this folder, before a publish replaces it (spec: LA-36).
    ///
    /// What stands there is stated without following a link (spec: EP-8): a
    /// symbolic link is kept aside as the link it is. A folder is
    /// [`Aside::Empty`], since a publish does not replace one. A file the
    /// filesystem will not link is [`Aside::Unkept`], carrying
    /// [`Linking`](crate::LocalOperation::Linking): what a caller does about a
    /// file it cannot put back is its own to decide, and nothing here copies
    /// it instead. A regular file kept aside is locked for as long as the
    /// value handed back lives, the way a scratch being written is, so a sync
    /// in another process that meets it under its scratch name leaves it
    /// alone (spec: OC-8).
    ///
    /// Synchronous, for the reason [`create`](Self::create) is.
    ///
    /// # Errors
    ///
    /// [`BelowRootError::Io`] where what stands at the name could not be
    /// stated, or the link was refused for a reason other than the filesystem
    /// not making one — the folder not writable, the disk full — or the kept
    /// file could not be locked; nothing is left under `kept_name` then.
    fn keep_aside(&self, kept_name: &str) -> Result<Aside, BelowRootError>;

    /// Where one of this folder's files stands, for a message to name.
    ///
    /// Nothing reaches the filesystem through it: every call that does is made
    /// against the open folder, by name.
    fn path_of(&self, name: &str) -> PathBuf;
}
