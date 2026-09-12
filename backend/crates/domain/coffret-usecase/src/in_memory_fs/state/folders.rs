use std::io;
use std::path::{Path, PathBuf};

use crate::descent_error::DescentError;
use crate::device_state::RootMarkerId;
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
    ///
    /// The root is *not* made, and nothing below it is made until the root has
    /// proved to be the one the mapping was recorded against: a folder no
    /// registration ever visited carries no marker, so placing into one is the
    /// very thing the check refuses (spec: EP-13).
    pub(in crate::in_memory_fs) fn reach(
        &mut self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        folders: &[String],
    ) -> Result<PathBuf, DescentError> {
        if !self.holds(root) {
            // What opening the root refuses, and not a verdict about the marker:
            // which folder this is is a question about a folder that is there
            // (spec: EP-12, EP-13).
            return Err(missing_root(root));
        }
        if !self.is_dir(root) {
            // The fence the real descent answers with, since nothing was made
            // here either: opening the root with `O_DIRECTORY` reports `ENOTDIR`
            // and the gateway reads that as a path this device cannot
            // materialize (spec: EP-4, EP-11). `walk` below says the same of the
            // same root, so the fake does not have one verdict for a read and
            // another for a write.
            return Err(DescentError::Blocked {
                stopped_at: root.to_path_buf(),
            });
        }
        self.vouch(root, expected)?;

        let mut folder = root.to_path_buf();
        for step in folders {
            folder.push(step);
            if self.is_dir(&folder) {
                continue;
            }
            if self.holds(&folder) {
                return Err(DescentError::Blocked { stopped_at: folder });
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
                stopped_at: root.to_path_buf(),
            });
        }

        let mut folder = root.to_path_buf();
        for step in folders {
            folder.push(step);
            if self.is_dir(&folder) {
                continue;
            }
            if self.holds(&folder) {
                return Err(DescentError::Blocked { stopped_at: folder });
            }
            return Ok(None);
        }
        Ok(Some(folder))
    }
}

/// What a mapped root that is not there at all refuses with.
///
/// An I/O refusal rather than a fence or a verdict about the root's identity,
/// because that is what the operating system would answer: a path nothing is at
/// is not a place a descent gets to ask which folder it is (spec: EP-12, EP-13).
/// Stated rather than created, for the reason the descent behind the real
/// capability states it: the root is opened and never made.
fn missing_root(root: &Path) -> DescentError {
    DescentError::Io(LocalIoError::new(
        LocalOperation::Stating,
        root,
        io::Error::other("the mapped root is not there"),
    ))
}
