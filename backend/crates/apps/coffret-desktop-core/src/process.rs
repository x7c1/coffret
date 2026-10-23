use std::ffi::OsStr;
use std::sync::{Mutex, OnceLock, PoisonError};

use crate::{idle_minutes, Desktop, Error, Result};

/// The one [`Desktop`] this process holds, once it has started.
static DESKTOP: OnceLock<Desktop> = OnceLock::new();

/// Whether the log has been started: once per process, whatever became of
/// the start it was part of.
static LOGGING: OnceLock<()> = OnceLock::new();

/// Held while the process is starting, so that two calls at once start one.
static STARTING: Mutex<()> = Mutex::new(());

/// Starts this process's desktop: the log, the one runtime, and the door.
///
/// `identifier` is the app's, which says whether this is the installed app or
/// the development one; `idle_minutes_setting` is what [`IDLE_MINUTES`] was
/// set to, which [`idle_minutes()`] reads. The log is started here and nowhere
/// else, and once per process: a start refused after it is running may be
/// tried again, and the log is not started a second time.
///
/// Refused once the process has started, rather than answered with the
/// desktop already there: a second start is a shell that lost track of the
/// first, and starting again would be a second runtime and a second door.
///
/// [`IDLE_MINUTES`]: crate::IDLE_MINUTES
pub fn start(identifier: &str, idle_minutes_setting: Option<&OsStr>) -> Result<&'static Desktop> {
    let _starting = STARTING.lock().unwrap_or_else(PoisonError::into_inner);
    if DESKTOP.get().is_some() {
        return Err(Error::AlreadyStarted);
    }
    let minutes = idle_minutes(idle_minutes_setting)?;
    if LOGGING.get().is_none() {
        coffret_shell::logging::start().map_err(|cause| Error::LogNotStarted { cause })?;
        let _ = LOGGING.set(());
    }
    let desktop = Desktop::new(identifier, minutes)?;
    Ok(DESKTOP.get_or_init(|| desktop))
}

/// This process's desktop, once it has started.
pub fn started() -> Option<&'static Desktop> {
    DESKTOP.get()
}
