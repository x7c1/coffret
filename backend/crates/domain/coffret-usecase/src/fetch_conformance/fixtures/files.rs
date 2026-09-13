use std::path::{Path, PathBuf};

use coffret_model::Mtime;

use crate::in_memory_fs::InMemoryFs;
use crate::scratch;

/// Puts a file in the fetching device's folder, making the folders above it.
///
/// The folder is in the same fake as the source device's, because what a case
/// arranges here is a place a fetch will find occupied: a file this device never
/// placed, or one it placed and no longer recognizes (spec: EP-10, EP-11). What
/// makes the arrangement worth making is that the capability under the placement
/// can be told to refuse, which no real directory can.
pub(crate) fn place(fs: &InMemoryFs, folder: &Path, relative: &str, content: &[u8]) -> PathBuf {
    let path = folder.join(relative);
    fs.write_file(&path, content);
    path
}

/// Content that differs in every byte, so a file assembled from the wrong
/// offsets lands on a different hash rather than on the same one.
pub(crate) fn filler(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// One local file's whole content, which the case expects to be there.
pub(crate) fn read(fs: &InMemoryFs, path: &Path) -> Vec<u8> {
    fs.content(path)
        .unwrap_or_else(|| panic!("a placed file must be there to read"))
}

/// Removes one file of that folder.
pub(crate) fn unplace(fs: &InMemoryFs, path: &Path) {
    assert!(
        fs.holds(path),
        "a case unplaces a file it or a run put there",
    );
    fs.remove_file(path);
}

/// Whether a local path holds anything at all.
///
/// Anything, and deliberately not "a file": a name a fetch may not write at is
/// one name whichever of a file, a folder, or something else is standing there
/// (spec: EP-8, EP-11).
pub(crate) fn exists(fs: &InMemoryFs, path: &Path) -> bool {
    fs.holds(path)
}

/// What the filesystem says about a local file now.
pub(crate) fn observed(fs: &InMemoryFs, path: &Path) -> (u64, Mtime) {
    fs.observed(path)
        .unwrap_or_else(|| panic!("a placed file must be there to state"))
}

/// How many of a fetch's scratches a folder still holds (spec: EP-11).
///
/// A run that placed nothing must also have left nothing: a half-written file
/// inside a mapped folder is exactly what the scratch-and-rename exists to keep
/// out of a reader's way. The whole subtree, because a scratch lands in
/// whichever folder of it its Entry belongs in.
pub(crate) fn scratch_left(fs: &InMemoryFs, folder: &Path) -> usize {
    fs.files_beneath(folder)
        .iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(scratch::is_scratch)
        })
        .count()
}
