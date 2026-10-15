//! How the app lets a browser into the explorer: a one-time launch ticket,
//! handed over in a file only this account can read (spec: LA-17, LA-21).
//!
//! The explorer's host admits a browser by a browser token alone, and a page
//! gets one by exchanging a launch ticket, which only this process can issue.
//! So every time the app opens the explorer — after the Library is opened, from
//! the tray, on a second launch — it issues a fresh ticket, writes it into a
//! launch file through the host's own writer, and asks the system to open that
//! file's `file:` URL ([`browser_target()`]). The browser's command line then
//! carries a path, and never the ticket.
//!
//! The files live in an owner-only directory of this app's own
//! ([`launch_dir_path()`]): under `$XDG_RUNTIME_DIR` on Linux, which the
//! session clears at logout, and under the state directory where that is unset
//! and on macOS. It is named for the app's identifier, so the installed app
//! and the development app never share one. What a run before left there is
//! swept away as the app starts.
//!
//! [`Launcher`] holds the files it wrote and removes each once its ticket is
//! spent — exchanged, expired, or retired — and is what *Disconnect all browser
//! tabs* and the app's end go through.

use coffret_explorer_host::launch_file::LaunchFileError;

mod browser_target;
pub use browser_target::browser_target;

mod launch_dir_path;
pub use launch_dir_path::launch_dir_path;

mod launcher;
pub use launcher::Launcher;

mod prepare_launch_dir;
pub use prepare_launch_dir::prepare_launch_dir;

#[cfg(test)]
mod tests;

/// The kind of file-system failure under `error`, for an event, or `none`.
fn io_error_kind(error: &LaunchFileError) -> String {
    match error {
        LaunchFileError::NotRemoved(cause) | LaunchFileError::Io { cause, .. } => {
            format!("{:?}", cause.kind())
        }
        _ => "none".to_owned(),
    }
}
