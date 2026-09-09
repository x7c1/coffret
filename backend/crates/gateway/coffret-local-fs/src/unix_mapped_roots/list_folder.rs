use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;

use coffret_usecase::{
    FolderEntry, FolderEntryKind, LocalIoError, LocalOperation, MappedRelativeLocation,
};
use rustix::fd::BorrowedFd;
use rustix::ffi::CStr;
use rustix::fs::{AtFlags, Dir, FileType, Stat};
use rustix::io::Errno;

use super::open_components::{local_error, open_folder};
use crate::local_times::{btime_from_raw, mtime_from_raw};

pub(super) async fn list_folder(
    root: &Path,
    relative: Option<&MappedRelativeLocation>,
) -> Result<Option<Vec<FolderEntry>>, LocalIoError> {
    let root = root.to_path_buf();
    let error_root = root.clone();
    let relative = relative.cloned();
    match tokio::task::spawn_blocking(move || list_confined(&root, relative.as_ref())).await {
        Ok(answer) => answer,
        Err(joined) => Err(LocalIoError::new(
            LocalOperation::Listing,
            error_root,
            io::Error::other(joined),
        )),
    }
}

fn list_confined(
    root: &Path,
    relative: Option<&MappedRelativeLocation>,
) -> Result<Option<Vec<FolderEntry>>, LocalIoError> {
    let path = relative.map_or_else(|| root.to_path_buf(), |part| root.join(part.to_path_buf()));
    let Some(directory) = open_folder(root, relative, LocalOperation::Listing)? else {
        return Ok(None);
    };
    let mut listing =
        Dir::new(directory).map_err(|cause| local_error(LocalOperation::Listing, &path, cause))?;
    let mut entries = Vec::new();
    while let Some(entry) = listing.read() {
        let entry = entry.map_err(|cause| local_error(LocalOperation::Listing, &path, cause))?;
        if entry.file_name().to_bytes() == b"." || entry.file_name().to_bytes() == b".." {
            continue;
        }
        let name = OsString::from_vec(entry.file_name().to_bytes().to_vec());
        let child = path.join(&name);
        let kind = match kind_at(
            listing
                .fd()
                .map_err(|cause| local_error(LocalOperation::Listing, &path, cause))?,
            entry.file_name(),
        ) {
            Ok(kind) => kind,
            Err(Errno::NOENT) => continue,
            Err(cause) => return Err(local_error(LocalOperation::Stating, child, cause)),
        };
        entries.push(FolderEntry { name, kind });
    }
    Ok(Some(entries))
}

fn kind_of(stat: &Stat) -> FolderEntryKind {
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::Directory => FolderEntryKind::Folder,
        FileType::RegularFile => FolderEntryKind::File {
            size: length(stat.st_size),
            mtime: mtime_from_raw(stat.st_mtime, stat.st_mtime_nsec),
            btime: birth_time(stat),
        },
        _ => FolderEntryKind::Other,
    }
}

#[cfg(target_os = "linux")]
fn kind_at(directory: BorrowedFd<'_>, name: &CStr) -> Result<FolderEntryKind, Errno> {
    use rustix::fs::StatxFlags;

    match rustix::fs::statx(
        directory,
        name,
        AtFlags::SYMLINK_NOFOLLOW,
        StatxFlags::BASIC_STATS | StatxFlags::BTIME,
    ) {
        Ok(stat) => {
            let btime = (stat.stx_mask & StatxFlags::BTIME.bits() != 0)
                .then(|| btime_from_raw(stat.stx_btime.tv_sec, stat.stx_btime.tv_nsec));
            Ok(match FileType::from_raw_mode(stat.stx_mode as _) {
                FileType::Directory => FolderEntryKind::Folder,
                FileType::RegularFile => FolderEntryKind::File {
                    size: stat.stx_size,
                    mtime: mtime_from_raw(stat.stx_mtime.tv_sec, stat.stx_mtime.tv_nsec),
                    btime,
                },
                _ => FolderEntryKind::Other,
            })
        }
        Err(Errno::NOSYS) => rustix::fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
            .map(|stat| kind_of(&stat)),
        Err(cause) => Err(cause),
    }
}

#[cfg(not(target_os = "linux"))]
fn kind_at(directory: BorrowedFd<'_>, name: &CStr) -> Result<FolderEntryKind, Errno> {
    rustix::fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map(|stat| kind_of(&stat))
}

#[cfg(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
fn birth_time(stat: &Stat) -> Option<coffret_model::Btime> {
    Some(btime_from_raw(stat.st_birthtime, stat.st_birthtime_nsec))
}

#[cfg(not(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
)))]
fn birth_time(_stat: &Stat) -> Option<coffret_model::Btime> {
    None
}

fn length(raw: impl TryInto<u64>) -> u64 {
    raw.try_into().unwrap_or(0)
}
