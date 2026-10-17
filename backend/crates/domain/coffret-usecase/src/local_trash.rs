use std::path::Path;

use async_trait::async_trait;

use crate::descent_error::DescentError;
use crate::device_state::RootMarkerId;
use crate::mapped_relative_location::MappedRelativeLocation;

/// The desktop's own trash, as a sync or a deletion asks it to take one file.
///
/// A file this device materialized whose Entry has since left the Library, and
/// which still holds what this device last made it match, is discarded there by
/// the sync that finds it — or, where this device made the deletion, right
/// after its commit — rather than left to be carried back in as new
/// (spec: EP-15). Discarded and never deleted: the trash is where a person who meant
/// to keep it takes it back from.
///
/// A capability for the reason [`Spool`](crate::Spool),
/// [`MappedRoots`](crate::MappedRoots) and
/// [`Destinations`](crate::Destinations) are. What EP-15 promises is a promise
/// about *failure* — a discard that is refused leaves the file where it is, is
/// reported, and is tried again by the next run — and a real trash cannot be
/// asked to refuse a chosen call; nor may a test reach the trash of the
/// desktop it runs on. The in-memory fake answers this in
/// tests, and the device's own filesystem gateway answers it with the
/// platform's trash: the freedesktop.org Trash on Linux, the Finder's on macOS.
///
/// It takes the mapped root and the identity the mapping records for it, the
/// way [`Destinations::reach`](crate::Destinations::reach) does, and for the
/// same reason: moving a file out of a mapped folder is a write into it, and a
/// write is made only where the root has proved to be the one the mapping was
/// recorded against (spec: EP-13). The location below the root is the one the
/// scan found the file at, spelled the way the folder spelled it (spec: EP-8),
/// and every folder on the way down is a real folder of the root — nothing is
/// discarded through a symbolic link.
///
/// The trait is object safe, so a flow holds `&dyn LocalTrash` and is written
/// once against the device's own trash and against the in-memory fake alike.
#[async_trait]
pub trait LocalTrash: Send + Sync {
    /// Discards the regular file at `relative` below `root` into the trash.
    ///
    /// # Errors
    ///
    /// The vocabulary [`reach`](crate::Destinations::reach) answers in, because
    /// it is the same descent asked for a different write:
    /// [`DescentError::Refused`] or [`DescentError::Unvouched`] where the root
    /// could not be vouched for (spec: EP-13), [`DescentError::Blocked`] where
    /// something on the way down is not a real folder of the root, and
    /// [`DescentError::Io`] for anything else, a file that is no longer there
    /// among them. That one carries the step that refused, and
    /// [`Discarding`](crate::LocalOperation::Discarding) where the step was the
    /// discard itself: what stands at the name is not a regular file, or the
    /// trash refused it, with the trash's own error kept whole as the cause.
    /// Whatever the refusal, the file is left exactly where it was: nothing
    /// here falls back on deleting it.
    async fn discard(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<(), DescentError>;

    /// Removes the folder at `relative` below `root` if it holds nothing at
    /// all, and answers whether it did.
    ///
    /// What a sync asks of each folder its discards may have emptied
    /// (spec: EP-15). Only a folder with nothing in it is removed — no file, no
    /// folder, no dotfile — and the check and the removal are one step, so a
    /// file that arrives in between keeps the folder. A folder that still holds
    /// something, or that is not there at all, is `Ok(false)`: neither is a
    /// refusal. The sync still asks each folder above it, and the same check
    /// decides each: one above a folder that stayed holds it and stays too.
    /// The root itself is never asked about; `relative` names at least one
    /// folder below it.
    ///
    /// The same descent as [`discard`](Self::discard), for the
    /// same reason: removing a folder is a write into the mapped folder.
    ///
    /// # Errors
    ///
    /// The vocabulary [`discard`](Self::discard) answers in, with
    /// [`Removing`](crate::LocalOperation::Removing) where the step was the
    /// removal itself; a name on the way down or at `relative` that is not a
    /// real folder — a symbolic link among them — is
    /// [`DescentError::Blocked`], and is never followed.
    async fn remove_emptied_folder(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<bool, DescentError>;
}
