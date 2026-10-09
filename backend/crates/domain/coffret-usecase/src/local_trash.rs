use std::path::Path;

use async_trait::async_trait;

use crate::descent_error::DescentError;
use crate::device_state::RootMarkerId;
use crate::mapped_relative_location::MappedRelativeLocation;

/// The desktop's own trash, as a sync asks it to take one file.
///
/// A file this device materialized whose Entry has since left the Library, and
/// which still holds what this device last made it match, is moved there by the
/// sync that finds it rather than left to be carried back in as new
/// (spec: EP-15). Moved and never deleted: the trash is where a person who
/// meant to keep it takes it back from.
///
/// A capability for the reason [`Spool`](crate::Spool),
/// [`MappedRoots`](crate::MappedRoots) and
/// [`Destinations`](crate::Destinations) are. What EP-15 promises is a promise
/// about *failure* — a move that is refused leaves the file where it is, is
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
/// moved through a symbolic link.
///
/// The trait is object safe, so a flow holds `&dyn LocalTrash` and is written
/// once against the device's own trash and against the in-memory fake alike.
#[async_trait]
pub trait LocalTrash: Send + Sync {
    /// Moves the regular file at `relative` below `root` into the trash.
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
    /// [`MovingToTrash`](crate::LocalOperation::MovingToTrash) where the step
    /// was the move itself: what stands at the name is not a regular file, or
    /// the trash refused it, with the trash's own error kept whole as the cause.
    /// Whatever the refusal, the file is left exactly where it was: nothing
    /// here falls back on deleting it.
    async fn move_to_trash(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) -> Result<(), DescentError>;
}
