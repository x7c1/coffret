use std::sync::{PoisonError, RwLock};

use tokio::sync::{Mutex, MutexGuard};

use super::Reconnect;

/// The reconnect this process is running or last ran.
///
/// Device state in the sense the run-tracking values beside it are: about this
/// process, gone when it is, never uploaded (spec: LA-12).
#[derive(Debug, Default)]
pub struct Reconnects {
    /// Whoever is starting a flow, so that two presses at once start one.
    starting: Mutex<()>,
    standing: RwLock<Option<Reconnect>>,
}

impl Reconnects {
    /// Nothing reconnected yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the last reconnect stands, and `None` where none has run.
    pub fn reported(&self) -> Option<Reconnect> {
        self.standing
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Waits for whoever is starting a flow, and takes the turn.
    pub(super) async fn turn(&self) -> MutexGuard<'_, ()> {
        self.starting.lock().await
    }

    /// The consent page a flow is waiting on, where one is.
    pub(super) fn waiting_on(&self) -> Option<String> {
        match self.reported() {
            Some(Reconnect::Waiting { url }) => Some(url),
            _ => None,
        }
    }

    /// Records where the reconnect stands now.
    pub(super) fn stands(&self, reconnect: Reconnect) {
        *self
            .standing
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(reconnect);
    }
}
