use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::log_settings::LogSettings;

mod cap;

mod create_directory;

mod existing_files;

mod files;
use files::Files;

mod record_writer;

mod start_file;

#[cfg(test)]
mod tests;

/// What every log file's name starts with.
const FILE_PREFIX: &str = "coffret-";
/// What every log file's name ends with.
const FILE_SUFFIX: &str = ".log";

/// A directory of log files whose total size has a ceiling.
///
/// Writes go to the newest file until it reaches its size, at which point the
/// next one starts and the oldest are deleted until what remains fits the
/// budget. That is the difference from rotating by period and keeping a count
/// of files: nothing bounds how much a single day writes, and the requirement
/// here is that logging can never grow without bound on disk.
///
/// Every write reaches the file system directly rather than sitting in a
/// buffer. Logging is not on any hot path — the events are Storage calls, which
/// are round trips to a network — and a buffer would mean the last events
/// before a crash, the ones worth having, are the ones lost.
#[derive(Clone)]
pub struct RotatingFiles {
    files: Arc<Mutex<Files>>,
}

impl RotatingFiles {
    /// Opens the directory and starts a file to write into.
    ///
    /// Files left by earlier runs count against the ceiling and are pruned like
    /// any others, so a program that is restarted often cannot accumulate a
    /// directory of them.
    pub fn open(settings: &LogSettings) -> io::Result<Self> {
        let files = Files::open(settings)?;

        Ok(Self {
            files: Arc::new(Mutex::new(files)),
        })
    }

    /// The file events are being written to at the moment.
    ///
    /// Worth reporting once at startup: the point of logging to a file is that
    /// somebody can go and read it, which they have to be able to find.
    pub fn current_path(&self) -> PathBuf {
        self.lock().current_path()
    }

    /// The files, whether or not a thread panicked while holding them.
    ///
    /// A poisoned lock means some other thread panicked mid-write. The worst
    /// that leaves behind is a half-written line, and refusing to log from then
    /// on would throw away the events describing what went wrong.
    fn lock(&self) -> MutexGuard<'_, Files> {
        self.files
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
