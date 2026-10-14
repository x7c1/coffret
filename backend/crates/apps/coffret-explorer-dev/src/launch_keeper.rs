use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coffret_explorer_host::credentials::Credentials;
use coffret_explorer_host::launch_file::{LaunchDir, LaunchFile, LaunchFileError};
use coffret_explorer_host::KeySource;
use sha2::{Digest, Sha256};

use crate::{Tick, LAUNCH_FILE};

/// How old a launch file's ticket may be before another takes its place: half
/// the minute a ticket is good for, so that a file opened at any moment has
/// half a minute left to be exchanged in (spec: LA-17).
pub const REFRESH_AFTER: Duration = Duration::from_secs(30);

/// How long asking whether the server answers waits.
const PROBE_TIMEOUT: Duration = Duration::from_millis(250);

/// Keeps one launch file ready for the development host, which has no tray to
/// open the explorer from (spec: LA-21).
///
/// Each [`tick`](Self::tick) looks at the server's key file and at the file it
/// last wrote:
///
/// - a key it has not seen before, once a server answers for it, is a server
///   of another run (spec: LA-4), and every session and ticket goes with the
///   one before (spec: LA-18): the generation is retired and a fresh file
///   written in its place. The first key it sees opens the first file the
///   same way, retiring nothing;
/// - a file whose ticket was spent — exchanged, expired or retired with its
///   generation — is removed, and one whose ticket is old is replaced, so
///   that what is there is always worth opening;
/// - and the next file is written only while the server answers. One that
///   stopped revoked every session as it was found gone (spec: LA-15), and a
///   file written for it would hand out a session it could not keep.
pub struct LaunchKeeper {
    credentials: Credentials,
    dir: LaunchDir,
    host: SocketAddr,
    server: SocketAddr,
    key: Arc<dyn KeySource>,
    /// The digest of the last key seen, and not the key itself.
    last_key: Option<[u8; 32]>,
    written: Option<Written>,
}

struct Written {
    file: LaunchFile,
    at: Instant,
}

impl LaunchKeeper {
    /// A keeper writing into `dir` the files that open the host at `host`,
    /// issuing from `credentials`, in front of the server at `server` whose key
    /// `key` reads. It writes nothing until it ticks.
    pub fn new(
        credentials: Credentials,
        dir: LaunchDir,
        host: SocketAddr,
        server: SocketAddr,
        key: Arc<dyn KeySource>,
    ) -> Self {
        Self {
            credentials,
            dir,
            host,
            server,
            key,
            last_key: None,
            written: None,
        }
    }

    /// The credentials the files are issued from.
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    /// Where the launch file is, whether or not one stands there now.
    pub fn path(&self) -> PathBuf {
        self.dir.path().join(LAUNCH_FILE)
    }

    /// One look, as of `now`.
    pub fn tick(&mut self, now: Instant) -> Result<Tick, LaunchFileError> {
        if let Some(tick) = self.after_a_new_key(now)? {
            return Ok(tick);
        }
        let stood = self.written.is_some();
        if let Some(written) = &self.written {
            let spent = written.file.is_spent(&self.credentials);
            if !spent && now.duration_since(written.at) < REFRESH_AFTER {
                return Ok(Tick::Quiet);
            }
            if spent {
                self.remove();
            }
        }
        self.when_answering(now, stood)
    }

    /// Removes the launch file, as the host stops.
    pub fn close(mut self) -> io::Result<()> {
        match self.written.take() {
            Some(written) => written.file.remove(),
            None => Ok(()),
        }
    }

    /// Where the key is one not seen before and a server answers for it:
    /// retires the generation, every session and ticket of the last server's
    /// run with it, and writes the file for this one. Where none answers,
    /// takes away any file that stands.
    fn after_a_new_key(&mut self, now: Instant) -> Result<Option<Tick>, LaunchFileError> {
        let Some(key) = self.read_key() else {
            return Ok(None);
        };
        if self.last_key == Some(key) {
            return Ok(None);
        }
        if !self.answers() {
            // A key no server answers for: one that has not bound yet, or one
            // a server that stopped left behind in the Library's directory.
            // It is not taken as seen, so it is looked at again, and a file
            // that stands was for a run that is over.
            let stood = self.written.is_some();
            self.remove();
            return Ok(Some(if stood { Tick::Withdrawn } else { Tick::Quiet }));
        }
        let first = self.last_key.is_none();
        self.last_key = Some(key);
        if !first {
            self.credentials.retire();
        }
        self.write(now)?;
        let path = self.path();
        Ok(Some(if first {
            Tick::Opened(path)
        } else {
            Tick::ServerChanged(path)
        }))
    }

    /// Writes the next file where the server answers, and otherwise takes any
    /// file that stands away: [`Tick::Withdrawn`] where one `stood` at the
    /// start of the look.
    ///
    /// The key is read again once the server answered: a server publishes its
    /// key before it binds, so one that answered a moment after the last read
    /// may be a new run whose key that read did not see.
    fn when_answering(&mut self, now: Instant, stood: bool) -> Result<Tick, LaunchFileError> {
        let answering = self.last_key.is_some() && self.answers();
        if !answering {
            self.remove();
            return Ok(if stood { Tick::Withdrawn } else { Tick::Quiet });
        }
        if let Some(tick) = self.after_a_new_key(now)? {
            return Ok(tick);
        }
        self.write(now)?;
        Ok(Tick::Rearmed)
    }

    /// Writes a fresh file over whatever stands at the path.
    fn write(&mut self, now: Instant) -> Result<(), LaunchFileError> {
        let file = self
            .dir
            .deliver(LAUNCH_FILE, self.host, &self.credentials)?;
        // The file before, if any, was replaced by the rename; dropping it
        // leaves the new one where it is.
        self.written = Some(Written { file, at: now });
        Ok(())
    }

    /// Takes the file that stands away. One that will not go holds a ticket
    /// that is spent all the same (spec: LA-21).
    fn remove(&mut self) {
        if let Some(written) = self.written.take() {
            if let Err(cause) = written.file.remove() {
                tracing::warn!(
                    io_error_kind = ?cause.kind(),
                    "a spent launch file could not be removed",
                );
            }
        }
    }

    /// Whether the server's port takes a connection now.
    fn answers(&self) -> bool {
        TcpStream::connect_timeout(&self.server, PROBE_TIMEOUT).is_ok()
    }

    /// The digest of the key in the file now, and `None` where there is none
    /// to read.
    fn read_key(&self) -> Option<[u8; 32]> {
        let key = self.key.read().ok()?;
        (!key.is_empty()).then(|| Sha256::digest(key.as_bytes()).into())
    }
}
