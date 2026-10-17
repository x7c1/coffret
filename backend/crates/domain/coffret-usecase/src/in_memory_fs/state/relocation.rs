use std::io;
use std::path::{Path, PathBuf};

use crate::descent_error::DescentError;
use crate::device_state::RootMarkerId;
use crate::in_memory_fs::state::{under, State};
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::mapped_relative_location::MappedRelativeLocation;
use crate::relocate_error::RelocateError;

impl State {
    /// Relocates one regular file below a vouched root to another name below it,
    /// making the folders the new name needs and replacing nothing
    /// (spec: EP-8, EP-10, EP-13).
    ///
    /// The real one's order: the scripted refusal first, so a refused relocation
    /// leaves the file where it was; then the root and its marker; then every
    /// folder on the way to the file, each of which has to be a folder, and
    /// the file itself; then the folders on the way to the new name, made where
    /// they are missing; and last the name itself, which has to be free and on
    /// the file's own filesystem, and that filesystem one that offers a rename
    /// replacing nothing. Folders made for a relocation that is then refused
    /// are removed again.
    pub(in crate::in_memory_fs) fn relocate(
        &mut self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        from: &MappedRelativeLocation,
        to: &MappedRelativeLocation,
    ) -> Result<(), RelocateError> {
        let source = root.join(from.to_path_buf());
        let target = root.join(to.to_path_buf());
        self.attempt(LocalOperation::Relocating, &source)
            .map_err(DescentError::Io)?;
        self.vouched_root(root, expected)?;
        self.regular_file(root, from)?;

        let mut made = Vec::new();
        let mut folder = root.to_path_buf();
        let components: Vec<_> = to.components().collect();
        let (_, folders) = components
            .split_last()
            .expect("a file below a mapped root names at least itself");
        for step in folders {
            folder.push(step);
            if self.is_dir(&folder) {
                continue;
            }
            if self.holds(&folder) {
                self.unmake(&made);
                return Err(DescentError::Blocked { stopped_at: folder }.into());
            }
            self.dirs.insert(folder.clone());
            made.push(folder.clone());
        }
        if self.holds(&target) {
            self.unmake(&made);
            return Err(RelocateError::Taken { at: target });
        }
        if self.filesystem_of(&source) != self.filesystem_of(&target) {
            self.unmake(&made);
            return Err(RelocateError::AnotherFilesystem {
                from: source,
                to: target,
            });
        }
        if let Err(refused) = self.atomic_rename_offered(&source, &target) {
            self.unmake(&made);
            return Err(refused);
        }
        let node = self
            .files
            .remove(&source)
            .expect("the file was stated a moment ago");
        self.files.insert(target, node);
        Ok(())
    }

    /// Exchanges two regular files below a vouched root, each taking the
    /// other's name (spec: EP-10).
    pub(in crate::in_memory_fs) fn exchange(
        &mut self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        one: &MappedRelativeLocation,
        other: &MappedRelativeLocation,
    ) -> Result<(), RelocateError> {
        let first = root.join(one.to_path_buf());
        let second = root.join(other.to_path_buf());
        self.attempt(LocalOperation::Relocating, &first)
            .map_err(DescentError::Io)?;
        self.vouched_root(root, expected)?;
        self.regular_file(root, one)?;
        self.regular_file(root, other)?;
        if self.filesystem_of(&first) != self.filesystem_of(&second) {
            return Err(RelocateError::AnotherFilesystem {
                from: first,
                to: second,
            });
        }
        self.atomic_rename_offered(&first, &second)?;
        let a = self.files.remove(&first).expect("the file was stated");
        let b = self.files.remove(&second).expect("the file was stated");
        self.files.insert(first, b);
        self.files.insert(second, a);
        Ok(())
    }

    /// Marks the folder at `path` as the top of another filesystem.
    pub(in crate::in_memory_fs) fn mount(&mut self, path: &Path) {
        self.prepare_dir(path);
        self.mounts.insert(path.to_path_buf());
    }

    /// Marks the folder at `path` as the top of another filesystem, one that
    /// offers neither a rename replacing nothing nor an exchange.
    pub(in crate::in_memory_fs) fn mount_without_atomic_rename(&mut self, path: &Path) {
        self.mount(path);
        self.without_atomic_rename.insert(path.to_path_buf());
    }

    /// Makes the folder at `path` part of the filesystem above it again.
    pub(in crate::in_memory_fs) fn unmount(&mut self, path: &Path) {
        self.mounts.remove(path);
        self.without_atomic_rename.remove(path);
    }

    /// Takes away a planted name that is neither a file nor a folder.
    pub(in crate::in_memory_fs) fn remove_other(&mut self, path: &Path) {
        self.others.remove(path);
    }

    /// Which filesystem `path` stands on: the deepest mount above it, or the
    /// fake's own where none is.
    fn filesystem_of(&self, path: &Path) -> Option<PathBuf> {
        self.mounts
            .iter()
            .filter(|mount| under(path, mount))
            .max_by_key(|mount| mount.components().count())
            .cloned()
    }

    /// Whether the filesystem `from` stands on offers the rename asked of it,
    /// refusing the way the real one answers `EINVAL` where it does not
    /// (spec: EP-10, EP-11). Both names stand on one filesystem by now.
    fn atomic_rename_offered(&self, from: &Path, to: &Path) -> Result<(), RelocateError> {
        let offered = self
            .filesystem_of(from)
            .is_none_or(|mount| !self.without_atomic_rename.contains(&mount));
        if offered {
            return Ok(());
        }
        Err(RelocateError::NoAtomicRename {
            at: to.to_path_buf(),
            cause: io::Error::new(
                io::ErrorKind::Unsupported,
                "this filesystem offers no rename that replaces nothing",
            ),
        })
    }

    /// The root is a folder and vouches for itself (spec: EP-13).
    fn vouched_root(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
    ) -> Result<(), DescentError> {
        if !self.is_dir(root) {
            return Err(DescentError::Io(LocalIoError::new(
                LocalOperation::Stating,
                root,
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "the mapped root is not a folder here",
                ),
            )));
        }
        self.vouch(root, expected)
    }

    /// Every folder on the way to `relative` is a folder, and a regular file
    /// stands at it (spec: EP-8).
    fn regular_file(
        &self,
        root: &Path,
        relative: &MappedRelativeLocation,
    ) -> Result<(), DescentError> {
        let mut folder = root.to_path_buf();
        let components: Vec<_> = relative.components().collect();
        let (_, folders) = components
            .split_last()
            .expect("a file below a mapped root names at least itself");
        for step in folders {
            folder.push(step);
            if !self.is_dir(&folder) {
                if self.holds(&folder) {
                    return Err(DescentError::Blocked { stopped_at: folder });
                }
                return Err(missing(&folder));
            }
        }
        let path = root.join(relative.to_path_buf());
        if self.files.contains_key(&path) {
            return Ok(());
        }
        if self.holds(&path) {
            return Err(DescentError::Blocked { stopped_at: path });
        }
        Err(missing(&path))
    }

    /// Removes the folders a refused relocation made, the deepest first.
    fn unmake(&mut self, made: &[PathBuf]) {
        for folder in made.iter().rev() {
            self.dirs.remove(folder);
        }
    }
}

/// What a name nothing stands at refuses with.
fn missing(path: &Path) -> DescentError {
    DescentError::Io(LocalIoError::new(
        LocalOperation::Relocating,
        path,
        io::Error::new(io::ErrorKind::NotFound, "nothing stands at this path"),
    ))
}
