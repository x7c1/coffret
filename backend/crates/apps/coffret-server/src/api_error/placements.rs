//! Placements this device will not make, of one file or of a whole drop.

use coffret_device::Redacted;

use super::{ApiError, NO_FOLDER_HERE_SAID};

impl ApiError {
    /// Nowhere on this device stands for the subtree of the Library that was
    /// named (spec: EP-9).
    ///
    /// The same reason a fetch under an unmapped folder is declined for, said
    /// before anything is attempted rather than after, and so a refused
    /// placement rather than a declined one: a drop onto a folder this device
    /// has no folder for has nowhere to put a single one of its files, so the
    /// whole of it is refused at once instead of once per file.
    pub fn no_folder_here() -> Self {
        Self::refused_placement("unmapped", NO_FOLDER_HERE_SAID.to_owned())
    }

    /// A mapped folder whose filesystem gives no file lock (spec: OC-8).
    ///
    /// The scratch a file is written under is locked so that a sync in another
    /// process leaves it alone, and a filesystem that gives no lock — a network
    /// share, most of the time — is refused rather than written without one.
    /// It is not the server failing: the request was answerable and the
    /// Library is intact, and what is wrong is where the folder is, so the
    /// sentence says what to change. It names no local path, as
    /// [`refused_root`](Self::refused_root) does not: the log carries the
    /// refusal's redacted rendering, and the folder is the one the request
    /// was aimed at.
    pub(crate) fn no_file_lock(cause: &impl Redacted) -> Self {
        Self::refused_placement(
            "no_file_lock",
            "the mapped folder could not be locked — its filesystem provides no file lock, so \
             nothing was written into it, and a mapped folder has to be on a local disk"
                .to_owned(),
        )
        .caused_by(cause.redacted())
    }

    /// A file would replace an Entry whose Container is a Pack (spec: PK-15).
    ///
    /// Carrying such a change in is read-modify-replace over the whole Pack,
    /// which coffret does not do yet (spec: PK-10, PK-11, PK-12) — so a sync
    /// would find the changed file, surface it, and leave the Pack byte for byte
    /// as it is. Writing the file anyway would leave it sitting in a mapped
    /// folder that nothing can ever carry into the Library, which is the one
    /// state a person must not be put in silently. The refusal is made before any
    /// byte is written, and it names the file it is about.
    pub fn pack_resident() -> Self {
        Self::refused_placement(
            "pack_resident",
            "the Library holds this file inside a Pack, and coffret cannot replace one of those \
             yet"
            .to_owned(),
        )
    }
}
