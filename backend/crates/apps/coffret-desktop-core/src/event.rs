/// What the core tells the shell it runs in, for the shell to act on.
///
/// Each is a kind and nothing else: no event carries a door token, a launch
/// ticket, an address or a URL (spec: EL-1, LA-25). Whatever the shell needs
/// next — a ticket for the explorer, the door's URL for an unlock — it asks
/// for, through a call of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A Library was opened through the door, and the explorer is served in
    /// front of it: the shell may close the door's window and show the
    /// explorer.
    LibraryOpened,
    /// The Library served was unlocked in place through the door (spec: DK-1).
    LibraryUnlocked,
    /// The explorer asked for the Library to be unlocked: the shell opens the
    /// door in its unlock mode, since the Passphrase is never taken in the
    /// explorer's page (spec: DK-10).
    UnlockRequested,
    /// A Passphrase window's token ended unused, after its last refused
    /// submission: that window can take no Passphrase any more, and says it
    /// has to be opened again. The shell loads it afresh, from a token of its
    /// own, the next time it brings it forward (spec: LA-24). Which window it
    /// was is not said, since that would take the token: a shell that keeps
    /// one Passphrase window knows it is that one.
    PassphraseWindowSpent,
    /// The server stopped. Every browser session, launch ticket and door token
    /// was discarded before this was sent, and nothing is served any more: the
    /// shell ends, as the core never ends the process itself.
    ServerStopped,
}

impl Event {
    /// The fixed word an event in the log records this by.
    pub fn word(self) -> &'static str {
        match self {
            Self::LibraryOpened => "library_opened",
            Self::LibraryUnlocked => "library_unlocked",
            Self::UnlockRequested => "unlock_requested",
            Self::PassphraseWindowSpent => "passphrase_window_spent",
            Self::ServerStopped => "server_stopped",
        }
    }
}
