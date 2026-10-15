use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use coffret_explorer_host::credentials::Credentials;
use coffret_explorer_host::launch_file::{LaunchDir, LaunchFile, LaunchFileError};

use super::io_error_kind;

/// The app's handle on the explorer's credentials, and the launch files it
/// delivered tickets in (spec: LA-17, LA-18, LA-21).
///
/// One for as long as the app serves its Library: the host and the server
/// behind it are started together in this process and end with it, and the
/// Library served is the one opened first, so the generation the credentials
/// began with lasts until [`retire`](Self::retire) — which the app calls when
/// the server stops and as it quits. An idle lock is not the end of one: the
/// sessions are kept, and the host refuses them data until the Library is
/// unlocked (spec: LA-14).
///
/// No event it records names a ticket, a URL or a file's path: only what
/// happened, and the kind of failure where one did (spec: EL-1).
pub struct Launcher {
    credentials: Credentials,
    dir: LaunchDir,
    host: SocketAddr,
    delivered: Mutex<Delivered>,
}

/// The files written and not yet removed, and the number the next is named by.
#[derive(Default)]
struct Delivered {
    files: Vec<LaunchFile>,
    next: u64,
}

impl Launcher {
    /// A launcher writing into `dir` the files that open the host at `host`,
    /// issuing from `credentials`. It writes nothing until it is asked to open.
    pub fn new(credentials: Credentials, dir: LaunchDir, host: SocketAddr) -> Self {
        Self {
            credentials,
            dir,
            host,
            delivered: Mutex::new(Delivered::default()),
        }
    }

    /// Issues a fresh ticket, writes it into a fresh launch file, and answers
    /// where that file is, for the browser to be asked to open.
    ///
    /// Each opening is a file of its own, so that a browser that has not read
    /// the last one yet still finds it whole.
    pub fn open(&self) -> Result<PathBuf, LaunchFileError> {
        let mut delivered = self.delivered();
        let name = format!("explorer-{}.html", delivered.next);
        delivered.next += 1;
        let file = match self.dir.deliver(&name, self.host, &self.credentials) {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!(
                    kind = "launch_file_not_written",
                    io_error_kind = io_error_kind(&error),
                    "a launch file could not be written",
                );
                return Err(error);
            }
        };
        tracing::info!(kind = "launch_file_written", "a launch file was written");
        let path = file.path().to_path_buf();
        delivered.files.push(file);
        Ok(path)
    }

    /// Removes every file whose ticket is spent — exchanged, expired, or
    /// retired — and answers how many are left (spec: LA-21).
    ///
    /// A file that will not go is said in the log and let go of: its ticket is
    /// spent all the same, and the next start sweeps the file away.
    pub fn tidy(&self) -> usize {
        let mut delivered = self.delivered();
        let (spent, live): (Vec<_>, Vec<_>) = delivered
            .files
            .drain(..)
            .partition(|file| file.is_spent(&self.credentials));
        delivered.files = live;
        for file in spent {
            match file.remove() {
                Ok(()) => tracing::info!(
                    kind = "launch_file_removed",
                    reason = "spent",
                    "a spent launch file was removed",
                ),
                Err(cause) => tracing::warn!(
                    kind = "launch_file_not_removed",
                    reason = "spent",
                    io_error_kind = ?cause.kind(),
                    "a spent launch file could not be removed",
                ),
            }
        }
        delivered.files.len()
    }

    /// Ends every session and spends every ticket not yet exchanged, and
    /// removes the files they were in: what *Disconnect all browser tabs* does
    /// (spec: LA-18).
    pub fn disconnect_all(&self) {
        self.credentials.revoke_all();
        self.tidy();
    }

    /// Discards every ticket and session and starts the next generation, and
    /// removes every file: what the app does when the server behind the host
    /// stops and as it quits (spec: LA-18).
    pub fn retire(&self) {
        self.credentials.retire();
        self.tidy();
    }

    /// The files written and not yet removed.
    ///
    /// A panic in another holder leaves a list that is still a list of files;
    /// it is taken as it stands rather than refusing every later opening.
    fn delivered(&self) -> MutexGuard<'_, Delivered> {
        self.delivered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
