use coffret_model::Mtime;
use rustix::fs::{AtFlags, FileType};
use rustix::io::Errno;

use super::ConfinedDir;
use crate::fetch::descent_error::DescentError;
use crate::fetch::standing::Standing;
use crate::local_operation::LocalOperation;

impl ConfinedDir {
    /// What stands at the file's own name, or `None` where nothing does.
    ///
    /// `AT_SYMLINK_NOFOLLOW`, for the reason a scan stats a directory entry that
    /// way: a symbolic link is not the file it points at (spec: EP-8), and a
    /// link standing at the target path is something in the way rather than an
    /// empty place.
    pub(in crate::fetch) fn look(&self) -> Result<Option<Standing>, DescentError> {
        let stat = match rustix::fs::statat(
            &self.directory,
            self.name.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => stat,
            Err(absent) if absent == Errno::NOENT => return Ok(None),
            Err(cause) => return Err(self.refused(&self.name, LocalOperation::Stating, cause)),
        };
        Ok(Some(Standing {
            size: length(stat.st_size),
            mtime: Mtime::from_unix_seconds(seconds(stat.st_mtime)),
            is_file: FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile,
        }))
    }
}

/// One `off_t` a filesystem reported, as the length a caller reasons in.
///
/// Generic because the width and the signedness of the field are the platform's
/// to choose, and a conversion written against one platform's choice is either a
/// compile error or a lint on the other's.
fn length(raw: impl TryInto<u64>) -> u64 {
    raw.try_into().unwrap_or(0)
}

/// One `time_t` a filesystem reported, as the whole seconds an [`Mtime`] holds.
///
/// Generic for the reason [`length`] is. What it cannot recover is a moment
/// before 1970 on a platform whose `time_t` is unsigned, which comes back as the
/// far end of the range instead; the cost is a fetch reporting such a file as
/// locally changed rather than skipping it, which leaves it alone either way
/// (spec: EP-11).
fn seconds(raw: impl TryInto<i64>) -> i64 {
    raw.try_into().unwrap_or(i64::MAX)
}
