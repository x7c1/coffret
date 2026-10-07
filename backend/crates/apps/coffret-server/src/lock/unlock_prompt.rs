use tokio::sync::mpsc::{self, error::TrySendError};

/// How a server asks the process it runs in to take the Passphrase again.
///
/// A server takes no Passphrase over its socket: a Passphrase typed into a page
/// would be a Passphrase carried through one (spec: DK-1, DK-10, LA-3, LA-6).
/// What it can do is ask whoever started it, where that is a process with a
/// prompt of its own — the desktop app, whose window does not echo and is not a
/// page. This is that asking, and it carries nothing: the other end is only
/// woken, puts its own window in front, and hands what it is given to
/// [`ServerState::unlock`](crate::ServerState::unlock).
///
/// A server started from the command line has none: nothing reads its terminal
/// after it starts.
#[derive(Debug, Clone)]
pub struct UnlockPrompt(mpsc::Sender<()>);

/// Whether a server's asking reached anybody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asked {
    /// The prompt was woken, or was already waking for an earlier asking.
    Woken,
    /// Nothing listens at the other end any more, so the asking went nowhere.
    Unheard,
}

impl UnlockPrompt {
    /// A prompt, and the end of it the process that owns the window waits on.
    ///
    /// One asking is held at a time and the rest are folded into it: the window
    /// comes forward once however many times it was asked for while it was
    /// coming, which is all a person needs from it.
    pub fn channel() -> (Self, mpsc::Receiver<()>) {
        let (sender, receiver) = mpsc::channel(1);
        (Self(sender), receiver)
    }

    /// Wakes the other end.
    pub(crate) fn ask(&self) -> Asked {
        match self.0.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => Asked::Woken,
            Err(TrySendError::Closed(())) => Asked::Unheard,
        }
    }
}
