use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::Path;

use super::files::Retired;
use super::{FILE_PREFIX, FILE_SUFFIX};

/// The log files already in the directory, oldest first.
///
/// Names sort in the order the files were started, since the timestamp in them
/// is fixed-width and most significant first.
pub(super) fn existing_files(directory: &Path) -> io::Result<VecDeque<Retired>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with(FILE_PREFIX) || !name.ends_with(FILE_SUFFIX) {
            continue;
        }
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        found.push((
            name.to_owned(),
            Retired {
                path: entry.path(),
                len: metadata.len(),
            },
        ));
    }

    found.sort_by(|(left, _), (right, _)| left.cmp(right));
    Ok(found.into_iter().map(|(_, file)| file).collect())
}
