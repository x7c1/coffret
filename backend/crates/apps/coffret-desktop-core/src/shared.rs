use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use coffret_explorer_host::{HostKind, PageSource};

use crate::door::Windows;
use crate::events::Events;
use crate::opened::Opened;
use crate::Event;

/// What the core holds for the whole of the process, shared between the
/// shell's calls and the tasks on the runtime: the door's handlers, the
/// server's watcher, and the prompt the server asks for an unlock through.
pub(crate) struct Shared {
    /// Which kind of host the explorer is served from, from the app's
    /// identifier.
    pub(crate) kind: HostKind,
    /// Where the explorer's page is served from.
    pub(crate) page: PageSource,
    /// How long the Library stays open while nobody wants it (spec: DK-4).
    pub(crate) idle_minutes: u64,
    /// Taken for as long as one attempt at opening or unlocking a Library is
    /// under way, so that a second submission waits for the first rather than
    /// racing it for the Library's lock or for its keys.
    pub(crate) opening: tokio::sync::Mutex<()>,
    /// What is served, once a Library is open. Set once: one Library is served
    /// for as long as the process runs, so a generation of the explorer's host
    /// ends by the server stopping or the shell quitting, never by the Library
    /// served changing (spec: LA-18).
    pub(crate) opened: OnceLock<Opened>,
    /// Whether the server has stopped, after which nothing is served.
    stopped: AtomicBool,
    /// The door's tokens, one per Passphrase window opened (spec: LA-23).
    pub(crate) windows: Windows,
    /// Where the shell is told what happened.
    pub(crate) events: Events,
}

impl Shared {
    pub(crate) fn new(kind: HostKind, page: PageSource, idle_minutes: u64) -> Self {
        Self {
            kind,
            page,
            idle_minutes,
            opening: tokio::sync::Mutex::new(()),
            opened: OnceLock::new(),
            stopped: AtomicBool::new(false),
            windows: Windows::default(),
            events: Events::default(),
        }
    }

    /// What is served, if a Library is open and its server has not stopped.
    pub(crate) fn served(&self) -> Option<&Opened> {
        if self.has_stopped() {
            return None;
        }
        self.opened.get()
    }

    /// Whether the server has stopped.
    pub(crate) fn has_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// Discards every browser session, launch ticket and door token, and
    /// starts the explorer host's next generation (spec: LA-18, LA-23).
    pub(crate) fn retire(&self) {
        if let Some(opened) = self.opened.get() {
            opened.credentials.retire();
        }
        self.windows.end_all();
    }

    /// What the core does when the server it serves stops: nothing it issued
    /// for that server outlasts it, and the shell is told, to end itself.
    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.retire();
        self.events.send(Event::ServerStopped);
    }
}
