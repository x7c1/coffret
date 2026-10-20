use coffret_desktop_core::Event;
use napi_derive::napi;

/// What the core tells the shell, for the shell to act on. Each is a kind and
/// nothing else: none carries a token, a ticket or a URL (spec: EL-1).
#[napi(string_enum)]
pub enum DesktopEvent {
    /// A Library was opened through the door; the explorer is served.
    LibraryOpened,
    /// The Library served was unlocked in place through the door.
    LibraryUnlocked,
    /// The explorer asked for the Library to be unlocked: open the door in
    /// its unlock mode.
    UnlockRequested,
    /// The server stopped, and every session, ticket and door token was
    /// discarded: the shell ends.
    ServerStopped,
}

impl From<Event> for DesktopEvent {
    fn from(event: Event) -> Self {
        match event {
            Event::LibraryOpened => Self::LibraryOpened,
            Event::LibraryUnlocked => Self::LibraryUnlocked,
            Event::UnlockRequested => Self::UnlockRequested,
            Event::ServerStopped => Self::ServerStopped,
        }
    }
}
