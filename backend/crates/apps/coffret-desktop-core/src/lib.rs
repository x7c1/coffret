//! What a desktop shell needs to open a Library and serve the explorer from
//! inside its own process, whatever the shell is written in: the Tauri app
//! takes the pieces it shares with this from here, and the Node-API binding
//! (`coffret-napi`) is a thin layer over [`Desktop`].
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
//! The explorer is served as the Tauri app serves it, minus the launch files:
//! the shell is handed tickets directly ([`Desktop::issue_ticket`]) and hands
//! them to the explorer's page in its fragment (spec: LA-17).

mod app_build;
pub use app_build::AppBuild;

mod attempt;

mod desktop;
pub use desktop::Desktop;

mod door;
pub use door::{DoorMode, DoorUrl};

mod entered_passphrase;
pub use entered_passphrase::EnteredPassphrase;

mod event;
pub use event::Event;

mod events;
pub use events::EventSink;

mod idle_minutes;
pub use idle_minutes::{idle_minutes, IDLE_MINUTES};

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

use anyhow::Context;
use coffret_device::LibraryDir;

/// The names of the Libraries on this device, for a Passphrase window to
/// choose between.
pub fn libraries() -> anyhow::Result<Vec<String>> {
    Ok(LibraryDir::on_this_device()
        .context("the Libraries on this device could not be listed")?
        .iter()
        .map(|dir| dir.name().to_owned())
        .collect())
}
