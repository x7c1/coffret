//! Starting a server: the sequence a process that serves a Library goes
//! through, in one place for every process that does.
//!
//! The command line is one such process and the desktop shell is another. Both
//! take the same steps in the same order — this server's hold on the Library
//! (spec: LA-8), then the Passphrase, then the Library, then the catalog caught
//! up with what the Library has become, then the key this run admits its
//! callers by, then a socket — and the order is the point: every refusal a
//! person acts on is met before anything is bound, so it is said once rather
//! than once per request. What differs between the two is only how the
//! Passphrase is asked for and where what happened is said, and both of those
//! are the caller's.

use std::time::Duration;

mod open;

mod serving;
pub use serving::Serving;

/// What a server is started with.
#[derive(Debug, Clone)]
pub struct Launch {
    /// The Library on this device to serve, by the name it was created under.
    pub library: String,
    /// The loopback port to listen on; `0` asks the operating system for any
    /// free one, and [`Serving::address`] says which it chose.
    pub port: u16,
    /// How many minutes in which nothing is read from or written to the
    /// Library pass before this server locks it (spec: DK-4).
    ///
    /// Minutes rather than a [`Duration`], because minutes are what a person
    /// chose and what they are told back; [`Launch::idle_interval`] is the
    /// conversion, saturating.
    pub idle_minutes: u64,
}

impl Launch {
    /// How long the Library stays open while nobody wants it.
    ///
    /// Saturating, because a number of minutes large enough to wrap the
    /// multiplication would otherwise become a tiny interval, and a server that
    /// locked itself at once because somebody asked for a million years is the
    /// opposite of what they asked for.
    pub fn idle_interval(&self) -> Duration {
        Duration::from_secs(self.idle_minutes.saturating_mul(60))
    }
}
