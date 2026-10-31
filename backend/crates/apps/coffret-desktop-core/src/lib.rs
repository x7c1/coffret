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

mod listed_library;
pub use listed_library::ListedLibrary;

use coffret_device::{Appearance, AppearanceFile, LibraryDir};

/// The Libraries on this device, for a Passphrase window to choose between:
/// each one's ID, and the name it is listed under.
///
/// The names are the copies each Library's settings keep (spec: FM-15), which
/// is all that can be read before an unlock; two Libraries may carry one name,
/// and the ID is what a window submits.
pub fn libraries() -> Result<Vec<ListedLibrary>> {
    Ok(LibraryDir::libraries()
        .map_err(|cause| Error::LibrariesUnlisted { cause })?
        .into_iter()
        .map(ListedLibrary::from)
        .collect())
}

/// Which set of colours this device's explorer is drawn in, as the server
/// keeps it in the state directory (spec: LA-38): `system`, `light` or
/// `dark`, and `system` where nothing was kept or what was kept cannot be
/// read.
///
/// For the shell, which paints the explorer's window in the ground of that set
/// before the page has drawn anything, so a dark explorer does not open on a
/// white window.
pub fn appearance() -> &'static str {
    word_for(AppearanceFile::of_this_device().read())
}

/// The word `appearance` is said in to the shell: the one the server's route
/// answers with.
fn word_for(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::System => "system",
        Appearance::Light => "light",
        Appearance::Dark => "dark",
    }
}

#[cfg(test)]
mod appearance_tests {
    use super::*;

    // The words are the route's own, so the shell and the page read one
    // vocabulary.
    #[test]
    fn each_choice_is_said_as_the_route_says_it() {
        for appearance in [Appearance::System, Appearance::Light, Appearance::Dark] {
            assert_eq!(
                serde_json::to_value(appearance).expect("an appearance serializes"),
                word_for(appearance),
            );
        }
    }
}
