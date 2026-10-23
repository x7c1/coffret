//! What a desktop shell needs to open a Library and serve the explorer from
//! inside its own process, whatever the shell is written in. The desktop app's
//! Electron shell reaches it through the Node-API binding (`coffret-napi`),
//! which is a thin layer over [`Desktop`].
//!
//! A process holds one [`Desktop`] ([`start`]), and with it one tokio runtime
//! that the server, the explorer's host and the Passphrase door all run on.
//! The shell asks it for a handful of things — the Libraries on this device, a
//! Passphrase window's URL, whether a Library is open, a launch ticket and the
//! explorer's address — and is told what happens through [`Event`]s. None of
//! those calls takes a Passphrase.
//!
//! The Passphrase goes through the Passphrase door alone: a loopback listener
//! of the core's own that serves the Passphrase page and takes what it
//! submits, so that the Passphrase never passes through the shell's own
//! process (spec: LA-22, DK-10). It is read from the request's body straight
//! into [`EnteredPassphrase`], where DK-7's claim over it begins.
//!
//! The explorer is served through `coffret-explorer-host`, with no launch file
//! written for it: the shell is handed tickets directly
//! ([`Desktop::issue_ticket`]) and hands them to the explorer's page, in its own
//! window, in the fragment of the URL it loads (spec: LA-17, LA-21).

mod app_build;
pub use app_build::AppBuild;

mod attempt;

mod desktop;
pub use desktop::Desktop;

/// What a drop the shell hands over is answered with, under the names the
/// binding reads it by.
pub use coffret_device::{Dropped, DroppedRow, NotTakenRow};

mod door;
pub use door::{DoorMode, DoorUrl};

mod drawn;
pub use drawn::Drawn;

mod entered_passphrase;
pub use entered_passphrase::EnteredPassphrase;

mod error;
pub use error::{Error, Result};

mod event;
pub use event::Event;

mod events;
pub use events::EventSink;

mod idle_minutes;
pub use idle_minutes::{idle_minutes, IDLE_MINUTES};

mod io_step;
pub use io_step::IoStep;

mod open_library;
mod opened;

mod page_source;
pub use page_source::PAGE_DIR;

mod process;
pub use process::{start, started};

mod refusal_line;
pub use refusal_line::{refusal_evidence, refusal_line};

mod served_library;
pub use served_library::ServedLibrary;

mod shared;

#[cfg(test)]
mod testing;

use coffret_device::LibraryDir;

/// The names of the Libraries on this device, for a Passphrase window to
/// choose between.
pub fn libraries() -> Result<Vec<String>> {
    Ok(LibraryDir::on_this_device()
        .map_err(|cause| Error::LibrariesUnlisted { cause })?
        .iter()
        .map(|dir| dir.name().to_owned())
        .collect())
}
