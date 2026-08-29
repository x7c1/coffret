use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use coffret_model::Mtime;

use crate::scratch;

/// Writes a file under a folder, making the directories above it.
pub(crate) async fn write(folder: &Path, relative: &str, content: &[u8]) -> PathBuf {
    let path = folder.join(relative);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .expect("making a folder must succeed");
    }
    tokio::fs::write(&path, content)
        .await
        .expect("writing a file must succeed");
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
pub(crate) async fn read(path: &Path) -> Vec<u8> {
    tokio::fs::read(path)
        .await
        .unwrap_or_else(|error| panic!("reading a placed file must succeed: {error}"))
}

/// Whether a local path holds anything at all.
pub(crate) async fn exists(path: &Path) -> bool {
    tokio::fs::symlink_metadata(path).await.is_ok()
}

/// What the filesystem says about a local file now.
pub(crate) async fn observed(path: &Path) -> (u64, Mtime) {
    let metadata = tokio::fs::metadata(path)
        .await
        .unwrap_or_else(|error| panic!("stating a placed file must succeed: {error}"));
    let modified = metadata
        .modified()
        .expect("the filesystem keeps modification times")
        .duration_since(UNIX_EPOCH)
        .expect("the suite's Entries are stamped after the epoch");
    (
        metadata.len(),
        Mtime::from_unix_seconds(modified.as_secs() as i64),
    )
}

/// How many of a fetch's temporary files a folder still holds (spec: EP-11).
///
/// A run that placed nothing must also have left nothing: a half-written file
/// inside a mapped folder is exactly what the temp-and-rename exists to keep out
/// of a reader's way.
pub(crate) async fn scratch_left(folder: &Path) -> usize {
    let mut left = 0;
    let mut stack = vec![folder.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let mut listing = match tokio::fs::read_dir(&directory).await {
            Ok(listing) => listing,
            // A folder a failed run never made holds nothing.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("listing a folder must succeed: {error}"),
        };
        while let Some(entry) = listing
            .next_entry()
            .await
            .expect("listing a folder must succeed")
        {
            let name = entry.file_name();
            let name = name.to_str().expect("the suite writes UTF-8 names");
            if entry
                .file_type()
                .await
                .expect("stating a folder entry must succeed")
                .is_dir()
            {
                stack.push(entry.path());
            } else if scratch::is_scratch(name) {
                left += 1;
            }
        }
    }
    left
}
