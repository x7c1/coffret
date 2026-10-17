use std::path::Path;

use async_trait::async_trait;

use crate::device_state::RootMarkerId;
use crate::mapped_relative_location::MappedRelativeLocation;
use crate::relocate_error::RelocateError;

/// Where a file this device placed goes when a rename has given its Entry
/// another current path: one rename on this device's own disk, from where the
/// file stands to where its Entry now does (spec: EP-10).
///
/// A capability for the reason [`LocalTrash`](crate::LocalTrash) and
/// [`Destinations`](crate::Destinations) are. What a relocation promises is a
/// promise about *failure* and *interruption* — a refused one leaves the file
/// and its record exactly where they were, and an interrupted one is finished
/// by the next run without overwriting or losing a file — and a real
/// filesystem cannot be asked to refuse a chosen call. The in-memory fake
/// answers this in tests, and the device's own filesystem gateway answers it
/// with the descriptor-relative rename the platform offers.
///
/// It takes the mapped root and the identity the mapping records for it, the
/// way a [`discard`](crate::LocalTrash::discard) does, and for the same reason:
/// relocating a file inside a mapped folder is a write into it, made only where the
/// root has proved to be the one the mapping was recorded against
/// (spec: EP-13). Every folder on the way to either name is a real folder of
/// the root, entered without following a link (spec: EP-8), and nothing is
/// ever copied: both names are under one root, and a rename that would cross
/// onto another filesystem is refused rather than made by copying and deleting.
///
/// The trait is object safe, so a flow holds `&dyn Relocations` and is written
/// once against the device's own disk and against the in-memory fake alike.
#[async_trait]
pub trait Relocations: Send + Sync {
    /// Relocates the regular file at `from` below `root` to `to` below the same
    /// root, making the folders `to` needs, with one rename that never replaces
    /// anything standing at `to`.
    ///
    /// `from` is spelled the way the folder spells it, as a walk found the file
    /// (spec: EP-8); `to` is the Entry Path's components below the mapping, the
    /// way a placement names them. Folders the call made for `to` are removed
    /// again where the rename is refused, so a refusal leaves the disk as it
    /// was.
    ///
    /// # Errors
    ///
    /// [`RelocateError::Taken`] where something already stands at `to`,
    /// [`RelocateError::AnotherFilesystem`] where the two names stand on two
    /// filesystems, [`RelocateError::NoAtomicRename`] where the filesystem
    /// offers no rename that never replaces what stands at `to`, and
    /// [`RelocateError::Descent`] for the root that would not
    /// vouch for itself, a name on the way to either path that is not a real
    /// folder of the root — a symbolic link among them — and anything the disk
    /// refused, a file no longer at `from` among them. Whatever the refusal,
    /// the file is left exactly where it was.
    async fn relocate(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        from: &MappedRelativeLocation,
        to: &MappedRelativeLocation,
    ) -> Result<(), RelocateError>;

    /// Exchanges the regular files at `one` and `other` below `root` with one
    /// rename, so each stands where the other stood and neither is ever
    /// replaced.
    ///
    /// What two files whose Entries swapped their names ask for, and what a
    /// longer cycle of such files is relocated by, one exchange at a time: no
    /// file of the cycle can be relocated first while another stands where it goes
    /// (spec: EP-10, FM-15). Both names are spelled the way the folder spells
    /// them.
    ///
    /// # Errors
    ///
    /// The vocabulary [`relocate`](Self::relocate) answers in, save
    /// [`RelocateError::Taken`]: both names are meant to be taken.
    async fn exchange(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        one: &MappedRelativeLocation,
        other: &MappedRelativeLocation,
    ) -> Result<(), RelocateError>;
}
