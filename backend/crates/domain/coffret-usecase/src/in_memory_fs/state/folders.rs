use std::io;
use std::path::{Path, PathBuf};

use crate::descent_error::DescentError;
use crate::in_memory_fs::state::{under, State};
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;

impl State {
    /// Records `dir` and every directory above it.
    pub(in crate::in_memory_fs) fn prepare_dir(&mut self, dir: &Path) {
        let mut above = Some(dir);
        while let Some(one) = above {
            self.dirs.insert(one.to_path_buf());
            above = one.parent();
        }
    }

    /// Removes one folder and everything under it.
    pub(in crate::in_memory_fs) fn remove_dir_all(&mut self, dir: &Path) {
        self.dirs.retain(|held| !under(held, dir));
        self.files.retain(|held, _| !under(held, dir));
        self.others.retain(|held| !under(held, dir));
    }

    /// Walks from a mapped root to the folder one file belongs in, making the
    /// folders that are not there yet.
    ///
    /// `folders` is every component but the file's own name. Each one that is
    /// missing is made and each one that is something other than a folder is a
    /// refusal, which is the whole of what the real descent's `O_DIRECTORY` and
    /// `O_NOFOLLOW` say (spec: EP-4, EP-11) — a planted "other" standing in for
    /// the symbolic link there is no filesystem here to make.
    pub(in crate::in_memory_fs) fn reach(
        &mut self,
        root: &Path,
        folders: &[String],
    ) -> Result<PathBuf, DescentError> {
        if self.holds(root) && !self.is_dir(root) {
            // What making the folder would refuse: something is at the path and
            // it is not one.
            return Err(DescentError::Io(LocalIoError::new(
                LocalOperation::Creating,
                root,
                io::Error::other("what is at the mapped root is not a folder"),
            )));
        }
        self.prepare_dir(root);

        let mut folder = root.to_path_buf();
        for step in folders {
            folder.push(step);
            if self.is_dir(&folder) {
                continue;
            }
            if self.holds(&folder) {
                return Err(DescentError::Blocked { path: folder });
            }
            self.dirs.insert(folder.clone());
        }
        Ok(folder)
    }

    /// The same walk over the folders that are already there, making none.
    ///
    /// `None` where a folder on the way is not there at all: nothing can stand
    /// at the file's path if the folder above it does not exist, which is the
    /// same answer as an empty place rather than a refusal.
    pub(in crate::in_memory_fs) fn walk(
        &self,
        root: &Path,
        folders: &[String],
    ) -> Result<Option<PathBuf>, DescentError> {
        if !self.holds(root) {
            return Ok(None);
        }
        if !self.is_dir(root) {
            return Err(DescentError::Blocked {
                path: root.to_path_buf(),
            });
        }

        let mut folder = root.to_path_buf();
        for step in folders {
            folder.push(step);
            if self.is_dir(&folder) {
                continue;
            }
            if self.holds(&folder) {
                return Err(DescentError::Blocked { path: folder });
            }
            return Ok(None);
        }
        Ok(Some(folder))
    }
}
