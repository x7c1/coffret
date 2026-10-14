//! The links in the explorer's build output, which the build refuses to embed.
//!
//! Shared by `build.rs`, which runs it, and by the crate's tests, which hold it
//! to finding one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Every symbolic link under `dir`, which is not followed into.
///
/// Embedding follows a link to whatever it points at, so a link in the build
/// output would put a file from outside it into the binary, to be served as
/// part of the page.
pub fn links_in(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut links = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                links.push(entry.path());
            } else if kind.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    links.sort();
    Ok(links)
}
