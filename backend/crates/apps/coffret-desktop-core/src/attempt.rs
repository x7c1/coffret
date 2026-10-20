use std::sync::Arc;

use coffret_device::open_library as reopen;
use coffret_device::Passphrase;
use coffret_server::Unlocked;

use crate::door::DoorMode;
use crate::open_library::open_library;
use crate::shared::Shared;
use crate::{refusal_evidence, refusal_line, Event};

/// What a submission through the door did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Attempted {
    /// The Library was opened, and the explorer is served in front of it.
    Opened,
    /// The Library named was already open: a submission that waited behind
    /// the one that opened it.
    AlreadyOpen,
    /// The Library served was unlocked in place (spec: DK-1).
    Unlocked,
    /// It was already unlocked: a submission that waited behind the unlock
    /// that landed.
    AlreadyUnlocked,
}

/// Why a submission through the door did not open or unlock the Library.
#[derive(Debug)]
pub(crate) enum AttemptRefused {
    /// An opening that named no Library.
    NoLibraryNamed,
    /// An opening that named a Library other than the one already served.
    AnotherLibraryOpen,
    /// An unlock with no Library served to unlock.
    NothingToUnlock,
    /// The server has stopped, and nothing is served any more.
    ServerStopped,
    /// Opening or unlocking the Library refused: the Passphrase does not open
    /// it, another server serves it, nothing could listen.
    Library(anyhow::Error),
    /// The attempt ended without an answer.
    Panicked,
}

impl AttemptRefused {
    /// The one sentence the window shows, which never carries a path
    /// ([`refusal_line()`]).
    pub(crate) fn line(&self) -> String {
        match self {
            Self::NoLibraryNamed => "Choose the Library to open.".to_owned(),
            Self::AnotherLibraryOpen => "Another Library is already open in this app.".to_owned(),
            Self::NothingToUnlock => "No Library is open in this app to unlock.".to_owned(),
            Self::ServerStopped => {
                "The server has stopped; quit the app and open it again.".to_owned()
            }
            Self::Library(refused) => refusal_line(refused),
            Self::Panicked => {
                "Opening or unlocking the Library stopped without an answer.".to_owned()
            }
        }
    }

    /// What the log is told of it: a fixed word, or the device's own failure
    /// as its log-safe rendering ([`refusal_evidence()`]), never the chain's
    /// words (spec: EL-1).
    pub(crate) fn evidence(&self) -> String {
        match self {
            Self::NoLibraryNamed => "NoLibraryNamed".to_owned(),
            Self::AnotherLibraryOpen => "AnotherLibraryOpen".to_owned(),
            Self::NothingToUnlock => "NothingToUnlock".to_owned(),
            Self::ServerStopped => "ServerStopped".to_owned(),
            Self::Library(refused) => refusal_evidence(refused),
            Self::Panicked => "Panicked".to_owned(),
        }
    }
}

/// Opens the Library called `library`, or unlocks the one served, with
/// `passphrase`, as `mode` says: the one thing the door does with what a
/// window submits.
///
/// The caller holds `shared.opening`, so that of two submissions at once the
/// second sees what the first did. The shell is told of an opening and of an
/// unlock that landed ([`Event`]); a submission that found it already done is
/// answered as done and tells it nothing new.
pub(crate) async fn attempt(
    shared: Arc<Shared>,
    mode: DoorMode,
    library: Option<String>,
    passphrase: Passphrase,
) -> Result<Attempted, AttemptRefused> {
    if shared.has_stopped() {
        return Err(AttemptRefused::ServerStopped);
    }
    match mode {
        DoorMode::Open => {
            let name = library
                .filter(|name| !name.is_empty())
                .ok_or(AttemptRefused::NoLibraryNamed)?;
            if let Some(opened) = shared.served() {
                return if opened.state.name == name {
                    Ok(Attempted::AlreadyOpen)
                } else {
                    Err(AttemptRefused::AnotherLibraryOpen)
                };
            }
            open_library(&shared, name, passphrase)
                .await
                .map_err(AttemptRefused::Library)?;
            shared.events.send(Event::LibraryOpened);
            Ok(Attempted::Opened)
        }
        DoorMode::Unlock => {
            let state = shared
                .served()
                .map(|opened| Arc::clone(&opened.state))
                .ok_or(AttemptRefused::NothingToUnlock)?;
            if state.holds_library() {
                return Ok(Attempted::AlreadyUnlocked);
            }
            // The Library is reopened the way it was opened — the device
            // crate's `open_library`, handed the Passphrase in the same
            // closure — and what that produced is handed to the running
            // server, which holds it until the next lock and arms that lock
            // afresh (spec: DK-1).
            let name = state.name.clone();
            let reopened = reopen(&name, move || Ok(passphrase))
                .await
                .map_err(|refused| AttemptRefused::Library(refused.into()))?;
            match state.unlock(reopened).map_err(AttemptRefused::Library)? {
                Unlocked::Now => {
                    tracing::info!(operation = "unlock", "the Library was unlocked in place");
                    shared.events.send(Event::LibraryUnlocked);
                    Ok(Attempted::Unlocked)
                }
                Unlocked::Already => Ok(Attempted::AlreadyUnlocked),
            }
        }
    }
}
