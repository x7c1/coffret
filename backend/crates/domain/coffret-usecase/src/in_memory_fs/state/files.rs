use std::io;
use std::path::Path;

use crate::descent_error::DescentError;
use crate::in_memory_fs::state::{fresh, FileNode, State, DEFAULT_MTIME};
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;

impl State {
    /// Empties the file at `path`, or refuses because nothing made the
    /// directory above it.
    pub(in crate::in_memory_fs) fn create(&mut self, path: &Path) -> Result<(), LocalIoError> {
        let missing = match path.parent() {
            Some(parent) => !self.dirs.contains(parent),
            // A path with no parent at all is not one any spool step forms.
            None => true,
        };
        if missing {
            return Err(LocalIoError::new(
                LocalOperation::Creating,
                path,
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "no directory was prepared for this file",
                ),
            ));
        }
        self.files.insert(path.to_path_buf(), fresh());
        Ok(())
    }

    /// Appends to the file at `path`, the way a write to an open handle does.
    ///
    /// The handle outlives a removal on a real filesystem, so a file that is no
    /// longer in the map is written afresh rather than refused: what a case
    /// arranges is never a spool somebody deleted mid-write.
    pub(in crate::in_memory_fs) fn append(&mut self, path: &Path, bytes: &[u8]) {
        self.files
            .entry(path.to_path_buf())
            .or_insert_with(fresh)
            .bytes
            .extend_from_slice(bytes);
    }

    /// Puts a whole file at `path`, making the folders above it.
    ///
    /// What a case arranging a mapped folder calls, so the folders come along
    /// with it: a person writing a file into a folder that is not there makes
    /// the folder first, and a case that had to say so twice would only be
    /// spelling out the same arrangement.
    pub(in crate::in_memory_fs) fn write_file(&mut self, path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            self.prepare_dir(parent);
        }
        self.others.remove(path);
        self.files.insert(
            path.to_path_buf(),
            FileNode {
                bytes: bytes.to_vec(),
                mtime: DEFAULT_MTIME,
                btime: Some(DEFAULT_MTIME),
            },
        );
    }

    /// Moves a file's modification time without touching a byte of it.
    pub(in crate::in_memory_fs) fn set_mtime(&mut self, path: &Path, seconds: i64) {
        let file = self
            .files
            .get_mut(path)
            .expect("a case restamps a file it planted");
        file.mtime = seconds;
    }

    /// Sets, or clears, what the fake reports as a file's birth time.
    ///
    /// Clearing it is what a filesystem that keeps no creation time looks like,
    /// which is the shape an absent field on a record stands for (spec: FM-9).
    pub(in crate::in_memory_fs) fn set_btime(&mut self, path: &Path, seconds: Option<i64>) {
        let file = self
            .files
            .get_mut(path)
            .expect("a case restamps a file it planted");
        file.btime = seconds;
    }

    /// Plants a name that is neither a file nor a folder, making the folders
    /// above it.
    pub(in crate::in_memory_fs) fn plant_other(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            self.prepare_dir(parent);
        }
        self.files.remove(path);
        self.others.insert(path.to_path_buf());
    }

    /// Makes an empty file at `path`, refusing a name anything already stands
    /// at.
    ///
    /// Exclusive, which is what a scratch file is opened with: a name that is
    /// taken is a refusal rather than a file two writers share, and a planted
    /// "other" that took it is refused rather than written through
    /// (spec: EP-11).
    pub(in crate::in_memory_fs) fn create_new(&mut self, path: &Path) -> Result<(), DescentError> {
        if self.holds(path) {
            return Err(DescentError::Io(LocalIoError::new(
                LocalOperation::Creating,
                path,
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "something already stands at this name",
                ),
            )));
        }
        self.files.insert(path.to_path_buf(), fresh());
        Ok(())
    }

    /// Moves one file onto another name, replacing whatever stood there.
    ///
    /// What a rename within one folder does, which is the moment a fetched file
    /// exists (spec: EP-11). A name that is already a file is replaced, because
    /// that is what the real call does and what a placement means to do.
    pub(in crate::in_memory_fs) fn rename(&mut self, from: &Path, to: &Path) {
        let Some(node) = self.files.remove(from) else {
            return;
        };
        self.others.remove(to);
        self.files.insert(to.to_path_buf(), node);
    }

    /// Removes one file, absence being the same outcome (spec: OC-8).
    pub(in crate::in_memory_fs) fn remove(&mut self, path: &Path) {
        self.files.remove(path);
    }
}
