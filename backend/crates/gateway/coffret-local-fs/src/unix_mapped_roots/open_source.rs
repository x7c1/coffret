use std::ffi::OsStr;
use std::io;
use std::path::Path;

use coffret_usecase::{LocalIoError, LocalOperation, MappedRelativeLocation, SourceReader};
use rustix::fs::{FileType, Mode, OFlags};
use tokio::fs;

use super::open_components::{local_error, open_components};
use crate::unix_source_reader::UnixSourceReader;

pub(super) async fn open_source(
    root: &Path,
    relative: &MappedRelativeLocation,
) -> Result<Box<dyn SourceReader>, LocalIoError> {
    let root = root.to_path_buf();
    let error_root = root.clone();
    let relative = relative.clone();
    let path = root.join(relative.to_path_buf());
    let (file, bytes) =
        match tokio::task::spawn_blocking(move || open_confined(&root, &relative)).await {
            Ok(answer) => answer?,
            Err(joined) => {
                return Err(LocalIoError::new(
                    LocalOperation::Reading,
                    error_root,
                    io::Error::other(joined),
                ));
            }
        };
    Ok(Box::new(UnixSourceReader::new(
        fs::File::from_std(file),
        path,
        bytes,
    )))
}

fn open_confined(
    root: &Path,
    relative: &MappedRelativeLocation,
) -> Result<(std::fs::File, u64), LocalIoError> {
    let path = root.join(relative.to_path_buf());
    let mut components: Vec<&OsStr> = relative.components().collect();
    let name = components
        .pop()
        .expect("a mapped file location has a final component");
    let parent = match open_components(root, components.into_iter(), LocalOperation::Reading)? {
        Some(parent) => parent,
        None => {
            return Err(LocalIoError::new(
                LocalOperation::Reading,
                path,
                io::Error::from(io::ErrorKind::NotFound),
            ));
        }
    };
    let file = rustix::fs::openat(
        &parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|cause| local_error(LocalOperation::Reading, &path, cause))?;
    let stat = rustix::fs::fstat(&file)
        .map_err(|cause| local_error(LocalOperation::Reading, &path, cause))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(LocalIoError::new(
            LocalOperation::Reading,
            path,
            io::Error::other("what is at this path is not a regular file"),
        ));
    }
    Ok((std::fs::File::from(file), length(stat.st_size)))
}

fn length(raw: impl TryInto<u64>) -> u64 {
    raw.try_into().unwrap_or(0)
}
