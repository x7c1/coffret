use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use coffret_model::{Btime, Mtime};

use crate::device_state::RootIdentity;
use crate::folder_entry::FolderEntry;
use crate::folder_entry_kind::FolderEntryKind;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;

/// What a file the fake holds is, beyond its bytes.
///
/// The times are here rather than derived from a clock, because what every case
/// about them is asking is whether a *particular* time reached a record: a fake
/// that stamped "now" would make the assertion depend on when the case ran.
#[derive(Debug)]
struct FileNode {
    bytes: Vec<u8>,
    mtime: i64,
    btime: Option<i64>,
}

/// What the fake holds, and what it has been told to refuse.
///
/// Directories are kept apart from files for the one reason a spool needs them:
/// a real spool file cannot be created under a directory nobody made, and a fake
/// that let it be would answer a case about
/// [`prepare_dir`](crate::Spool::prepare_dir) with a success the device would
/// never give. The mapped-folder side needs them for a second reason — a folder
/// is something a listing reports and a walk descends into — so one set of
/// directories serves both.
///
/// `others` is what the fake has instead of a symbolic link: a name that is
/// neither a file nor a folder, so a case can arrange EP-8's rule without a real
/// filesystem to make a link on.
#[derive(Debug, Default)]
pub(super) struct State {
    dirs: BTreeSet<PathBuf>,
    files: BTreeMap<PathBuf, FileNode>,
    others: BTreeSet<PathBuf>,
    identities: BTreeMap<PathBuf, RootIdentity>,
    script: Vec<Fault>,
    attempts: BTreeMap<u8, usize>,
}

/// One scripted refusal: the `nth` invocation of `operation` fails.
#[derive(Debug)]
struct Fault {
    operation: LocalOperation,
    nth: usize,
}

/// The modification time a file the fake was handed carries until a case says
/// otherwise.
///
/// Fixed rather than taken from a clock, so that what a case writes into a
/// device's bookkeeping is the same on every machine and on every run.
const DEFAULT_MTIME: i64 = 1_700_000_000;

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
        self.files.insert(path.to_path_buf(), fresh());
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
    pub(super) fn write_file(&mut self, path: &Path, bytes: &[u8]) {
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
    pub(super) fn set_mtime(&mut self, path: &Path, seconds: i64) {
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
    pub(super) fn set_btime(&mut self, path: &Path, seconds: Option<i64>) {
        let file = self
            .files
            .get_mut(path)
            .expect("a case restamps a file it planted");
        file.btime = seconds;
    }

    /// Plants a name that is neither a file nor a folder, making the folders
    /// above it.
    pub(super) fn plant_other(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            self.prepare_dir(parent);
        }
        self.files.remove(path);
        self.others.insert(path.to_path_buf());
    }

    /// Records what [`probe_root`](crate::MappedRoots::probe_root) answers for
    /// one path.
    pub(super) fn set_root_identity(&mut self, path: &Path, identity: RootIdentity) {
        self.identities.insert(path.to_path_buf(), identity);
    }

    /// What the fake says the filesystem under `path` is.
    pub(super) fn root_identity(&self, path: &Path) -> Option<&RootIdentity> {
        self.identities.get(path)
    }

    /// One file's whole content, if it is there.
    pub(super) fn content(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.get(path).map(|file| file.bytes.clone())
    }

    /// One file's length and modification time, if it is there.
    pub(super) fn observed(&self, path: &Path) -> Option<(u64, Mtime)> {
        self.files.get(path).map(|file| {
            (
                file.bytes.len() as u64,
                Mtime::from_unix_seconds(file.mtime),
            )
        })
    }

    /// One file's birth time, if it is there and the fake reports one.
    pub(super) fn born(&self, path: &Path) -> Option<Btime> {
        self.files.get(path)?.btime.map(Btime::from_unix_seconds)
    }

    /// Removes one file, absence being the same outcome (spec: OC-6).
    pub(super) fn remove(&mut self, path: &Path) {
        self.files.remove(path);
    }

    /// Removes one folder and everything under it.
    pub(super) fn remove_dir_all(&mut self, dir: &Path) {
        self.dirs.retain(|held| !under(held, dir));
        self.files.retain(|held, _| !under(held, dir));
        self.others.retain(|held| !under(held, dir));
    }

    /// Whether anything at all stands at `path`.
    pub(super) fn holds(&self, path: &Path) -> bool {
        self.dirs.contains(path) || self.files.contains_key(path) || self.others.contains(path)
    }

    /// Whether `path` is a folder the fake holds.
    pub(super) fn is_dir(&self, path: &Path) -> bool {
        self.dirs.contains(path)
    }

    /// The names directly under `dir`, and what each of them stands for.
    ///
    /// Sorted, because a `BTreeMap` is what holds them; a real listing is in
    /// whatever order the filesystem felt like, and nothing above may depend on
    /// either.
    pub(super) fn list(&self, dir: &Path) -> Vec<FolderEntry> {
        let folders = self.dirs.iter().filter_map(|path| {
            entry(path, dir).map(|name| FolderEntry {
                name,
                kind: FolderEntryKind::Folder,
            })
        });
        let files = self.files.iter().filter_map(|(path, file)| {
            entry(path, dir).map(|name| FolderEntry {
                name,
                kind: FolderEntryKind::File {
                    size: file.bytes.len() as u64,
                    mtime: Mtime::from_unix_seconds(file.mtime),
                    btime: file.btime.map(Btime::from_unix_seconds),
                },
            })
        });
        let others = self.others.iter().filter_map(|path| {
            entry(path, dir).map(|name| FolderEntry {
                name,
                kind: FolderEntryKind::Other,
            })
        });
        folders.chain(files).chain(others).collect()
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

/// A file with no bytes and the fake's default stamps.
fn fresh() -> FileNode {
    FileNode {
        bytes: Vec::new(),
        mtime: DEFAULT_MTIME,
        btime: Some(DEFAULT_MTIME),
    }
}

/// The name `path` goes by inside `dir`, or `None` where it is not directly
/// inside it.
fn entry(path: &Path, dir: &Path) -> Option<OsString> {
    if path.parent() != Some(dir) {
        return None;
    }
    Some(path.file_name()?.to_os_string())
}

/// Whether `path` is `dir` itself or something beneath it.
///
/// Component-wise, which is what [`Path::starts_with`] is: a folder called
/// `photographs-old` is not under one called `photographs`.
fn under(path: &Path, dir: &Path) -> bool {
    path.starts_with(dir)
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
