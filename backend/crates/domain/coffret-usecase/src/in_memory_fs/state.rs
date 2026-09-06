use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;

/// What the fake holds, and what it has been told to refuse.
///
/// The directories are kept apart from the files for the one reason a fake
/// needs them at all: a real spool file cannot be created under a directory
/// nobody made, and a fake that let it be would answer a case about
/// [`prepare_dir`](crate::Spool::prepare_dir) with a success the device would
/// never give.
#[derive(Debug, Default)]
pub(super) struct State {
    dirs: BTreeSet<PathBuf>,
    files: BTreeMap<PathBuf, Vec<u8>>,
    script: Vec<Fault>,
    attempts: BTreeMap<u8, usize>,
}

/// One scripted refusal: the `nth` invocation of `operation` fails.
#[derive(Debug)]
struct Fault {
    operation: LocalOperation,
    nth: usize,
}

impl State {
    /// Scripts the `nth` (1-based) invocation of `operation` to fail.
    pub(super) fn fail_on(&mut self, operation: LocalOperation, nth: usize) {
        self.script.push(Fault { operation, nth });
    }

    /// Counts one invocation of `operation`, and refuses it where the script
    /// says this is the one.
    ///
    /// Every operation goes through here before it does anything, so a refusal
    /// leaves the fake exactly as the failure it stands for would: a creation
    /// that fails creates nothing, a write that fails writes nothing, a flush
    /// that fails leaves the bytes already written where they are.
    pub(super) fn attempt(
        &mut self,
        operation: LocalOperation,
        path: &Path,
    ) -> Result<(), LocalIoError> {
        let attempt = self.attempts.entry(code(operation)).or_default();
        *attempt += 1;
        let refused = self
            .script
            .iter()
            .any(|fault| code(fault.operation) == code(operation) && fault.nth == *attempt);
        if refused {
            return Err(LocalIoError::new(
                operation,
                path,
                io::Error::other("the fake filesystem was told to refuse this"),
            ));
        }
        Ok(())
    }

    /// Records `dir` and every directory above it.
    pub(super) fn prepare_dir(&mut self, dir: &Path) {
        let mut above = Some(dir);
        while let Some(one) = above {
            self.dirs.insert(one.to_path_buf());
            above = one.parent();
        }
    }

    /// Empties the file at `path`, or refuses because nothing made the
    /// directory above it.
    pub(super) fn create(&mut self, path: &Path) -> Result<(), LocalIoError> {
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
        self.files.insert(path.to_path_buf(), Vec::new());
        Ok(())
    }

    /// Appends to the file at `path`, the way a write to an open handle does.
    ///
    /// The handle outlives a removal on a real filesystem, so a file that is no
    /// longer in the map is written afresh rather than refused: what a case
    /// arranges is never a spool somebody deleted mid-write.
    pub(super) fn append(&mut self, path: &Path, bytes: &[u8]) {
        self.files
            .entry(path.to_path_buf())
            .or_default()
            .extend_from_slice(bytes);
    }

    /// One file's whole content, if it is there.
    pub(super) fn content(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.get(path).cloned()
    }

    /// Removes one file, absence being the same outcome (spec: OC-6).
    pub(super) fn remove(&mut self, path: &Path) {
        self.files.remove(path);
    }

    /// The files directly under `dir`, in path order.
    pub(super) fn files_under(&self, dir: &Path) -> Vec<PathBuf> {
        self.files
            .keys()
            .filter(|path| path.parent() == Some(dir))
            .cloned()
            .collect()
    }
}

/// Which counter one operation is tallied under.
///
/// [`LocalOperation`] is deliberately not [`Ord`] or [`Hash`] — it is a word for
/// a person, not a key — so the fake gives it one here. The match is exhaustive
/// on purpose: an operation added to the vocabulary is one this fake has to be
/// told what to do with rather than one that silently shares a counter.
fn code(operation: LocalOperation) -> u8 {
    match operation {
        LocalOperation::Listing => 0,
        LocalOperation::Stating => 1,
        LocalOperation::Reading => 2,
        LocalOperation::Creating => 3,
        LocalOperation::Writing => 4,
        LocalOperation::Flushing => 5,
        LocalOperation::Stamping => 6,
        LocalOperation::Renaming => 7,
        LocalOperation::Removing => 8,
    }
}
