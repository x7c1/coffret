use coffret_usecase::{DescentError, LocalOperation, Standing};
use rustix::fs::{AtFlags, FileType};
use rustix::io::Errno;

use crate::local_times::mtime_from_raw;
use crate::unix_destinations::open_folder::OpenFolder;

/// What stands at the file's own name, or `None` where nothing does.
///
/// `AT_SYMLINK_NOFOLLOW`, for the reason a scan stats a directory entry that
/// way: a symbolic link is not the file it points at (spec: EP-8), and a link
/// standing at the target path is something in the way rather than an empty
/// place.
pub(super) fn standing(folder: &OpenFolder) -> Result<Option<Standing>, DescentError> {
    let stat =
        match rustix::fs::statat(folder.directory(), folder.name(), AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(absent) if absent == Errno::NOENT => return Ok(None),
            Err(cause) => {
                return Err(folder.refused(folder.name(), LocalOperation::Stating, cause));
            }
        };
    Ok(Some(Standing {
        size: length(stat.st_size),
        mtime: mtime_from_raw(stat.st_mtime, stat.st_mtime_nsec),
        is_file: FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile,
    }))
}

/// One `off_t` a filesystem reported, as the length a caller reasons in.
///
/// Generic because the width and the signedness of the field are the platform's
/// to choose, and a conversion written against one platform's choice is either a
/// compile error or a lint on the other's.
fn length(raw: impl TryInto<u64>) -> u64 {
    raw.try_into().unwrap_or(0)
}
