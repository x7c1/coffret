use std::ffi::OsString;
use std::path::{Path, PathBuf};

use coffret_model::{Btime, Mtime};

use crate::folder_entry::FolderEntry;
use crate::folder_entry_kind::FolderEntryKind;
use crate::in_memory_fs::state::{under, State, DEFAULT_MTIME};
use crate::standing::Standing;

impl State {
    /// One file's whole content, if it is there.
    pub(in crate::in_memory_fs) fn content(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.get(path).map(|file| file.bytes.clone())
    }

    /// One file's length and modification time, if it is there.
    pub(in crate::in_memory_fs) fn observed(&self, path: &Path) -> Option<(u64, Mtime)> {
        self.files.get(path).map(|file| {
            (
                file.bytes.len() as u64,
                Mtime::from_unix_seconds(file.mtime),
            )
        })
    }

    /// One file's birth time, if it is there and the fake reports one.
    pub(in crate::in_memory_fs) fn born(&self, path: &Path) -> Option<Btime> {
        self.files.get(path)?.btime.map(Btime::from_unix_seconds)
    }

    /// Whether anything at all stands at `path`.
    pub(in crate::in_memory_fs) fn holds(&self, path: &Path) -> bool {
        self.dirs.contains(path) || self.files.contains_key(path) || self.others.contains(path)
    }

    /// Whether `path` is a folder the fake holds.
    pub(in crate::in_memory_fs) fn is_dir(&self, path: &Path) -> bool {
        self.dirs.contains(path)
    }

    /// The names directly under `dir`, and what each of them stands for.
    ///
    /// Sorted, because a `BTreeMap` is what holds them; a real listing is in
    /// whatever order the filesystem felt like, and nothing above may depend on
    /// either.
    pub(in crate::in_memory_fs) fn list(&self, dir: &Path) -> Vec<FolderEntry> {
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
    pub(in crate::in_memory_fs) fn files_under(&self, dir: &Path) -> Vec<PathBuf> {
        self.files
            .keys()
            .filter(|path| path.parent() == Some(dir))
            .cloned()
            .collect()
    }

    /// Every file anywhere beneath `dir`, in path order.
    ///
    /// What a case counting a fetch's leftovers reads: a temporary file lands in
    /// whichever folder of the destination tree its Entry belongs in, so the
    /// question is about the whole subtree rather than about one folder
    /// (spec: EP-11).
    pub(in crate::in_memory_fs) fn files_beneath(&self, dir: &Path) -> Vec<PathBuf> {
        self.files
            .keys()
            .filter(|path| under(path, dir) && path.as_path() != dir)
            .cloned()
            .collect()
    }

    /// What stands at `path`, with links unfollowed (spec: EP-8).
    ///
    /// A folder and a planted "other" are both *something in the way* rather
    /// than an empty place, so each comes back with `is_file` false — the
    /// reading the fetch's selection makes of one (spec: EP-11).
    pub(in crate::in_memory_fs) fn standing(&self, path: &Path) -> Option<Standing> {
        if let Some(file) = self.files.get(path) {
            return Some(Standing {
                size: file.bytes.len() as u64,
                mtime: Mtime::from_unix_seconds(file.mtime),
                is_file: true,
            });
        }
        if self.dirs.contains(path) || self.others.contains(path) {
            // Neither length nor time says anything about a name a file may not
            // be placed at, so the fake answers the same two values for both.
            return Some(Standing {
                size: 0,
                mtime: Mtime::from_unix_seconds(DEFAULT_MTIME),
                is_file: false,
            });
        }
        None
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
