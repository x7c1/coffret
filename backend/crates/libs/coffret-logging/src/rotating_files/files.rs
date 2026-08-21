use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;

use crate::log_settings::LogSettings;

use super::cap::cap;
use super::create_directory::create_directory;
use super::existing_files::existing_files;
use super::start_file::start_file;

/// How many files are kept, whatever they weigh.
///
/// The ceiling is on bytes, and bytes alone do not bound a directory: every run
/// starts a file, and a run that emits nothing leaves an empty one, which costs
/// no bytes and is therefore never the reason to prune. A program started often
/// enough would fill the directory with them. Generous, since the point is only
/// to have a bound at all.
pub(super) const MAX_FILES: usize = 32;

/// The files themselves, and the budget they are kept inside.
pub(super) struct Files {
    directory: PathBuf,
    max_total_bytes: u64,
    max_file_bytes: u64,
    current: Current,
    /// The files no longer written to, oldest first — the order they are
    /// dropped in.
    retired: VecDeque<Retired>,
}

/// The file being written to.
pub(super) struct Current {
    pub(super) path: PathBuf,
    pub(super) file: File,
    pub(super) len: u64,
}

/// A file that is kept only until the budget needs its bytes.
pub(super) struct Retired {
    pub(super) path: PathBuf,
    pub(super) len: u64,
}

impl Files {
    /// Takes the directory over, counting what earlier runs left in it.
    pub(super) fn open(settings: &LogSettings) -> io::Result<Self> {
        let directory = settings.directory().to_path_buf();
        let (max_total_bytes, max_file_bytes) = settings.sizes();

        create_directory(&directory)?;
        let retired = existing_files(&directory)?;
        let current = start_file(&directory)?;

        let mut files = Self {
            directory,
            max_total_bytes,
            max_file_bytes,
            current,
            retired,
        };
        files.prune();

        Ok(files)
    }

    /// The file being written to at the moment.
    pub(super) fn current_path(&self) -> PathBuf {
        self.current.path.clone()
    }

    /// Writes one formatted event, rotating and pruning as the budget requires.
    pub(super) fn write_record(&mut self, record: &[u8]) -> io::Result<()> {
        let record = cap(record, self.max_file_bytes);

        // A record only starts a new file if something is already in this one:
        // a record that fits nowhere still has to be written somewhere.
        if self.current.len > 0 && self.current.len + record.len() as u64 > self.max_file_bytes {
            self.roll()?;
        }

        self.current.file.write_all(&record)?;
        self.current.len += record.len() as u64;
        Ok(())
    }

    /// Pushes whatever the file system is still holding out to disk.
    pub(super) fn flush(&mut self) -> io::Result<()> {
        self.current.file.flush()
    }

    /// Starts the next file and brings the total back under the ceiling.
    fn roll(&mut self) -> io::Result<()> {
        let next = start_file(&self.directory)?;
        let previous = std::mem::replace(&mut self.current, next);
        self.retired.push_back(Retired {
            path: previous.path,
            len: previous.len,
        });
        self.prune();
        Ok(())
    }

    /// Drops the oldest files until the budget can hold one more full file, and
    /// until no more than [`MAX_FILES`] are left.
    ///
    /// Leaving room for a whole file rather than for what is in the current one
    /// is what makes the ceiling hold at every moment rather than only just
    /// after a prune: the current file is never a candidate to drop, and it may
    /// grow to its full size before the next roll.
    fn prune(&mut self) {
        let mut total: u64 = self.retired.iter().map(|file| file.len).sum();
        while total + self.max_file_bytes > self.max_total_bytes || self.retired.len() >= MAX_FILES
        {
            let Some(oldest) = self.retired.pop_front() else {
                return;
            };
            total -= oldest.len;
            // Best effort, and dropped from the accounting either way: a file
            // that will not delete is not going to delete on the next attempt
            // either, and retrying it forever would be a loop.
            let _ = fs::remove_file(&oldest.path);
        }
    }
}
