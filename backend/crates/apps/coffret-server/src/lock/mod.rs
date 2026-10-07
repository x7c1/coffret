//! The two states a served Library is in, and the moves between them.
//!
//! A device holds the Master Key locked or unlocked, and the Passphrase and a
//! lock are what move it between the two (spec: DK-1). On the command line
//! there is nothing to arrange: a command is one unlock and one run, and the
//! process ends holding nothing. A server does not end — so without this module
//! a person who walked away from their machine would leave something that opens
//! the whole Library for as long as it runs.
//!
//! # One cell, and emptying it is the lock
//!
//! Everything the derived keys reach lives behind [`Custody`], and a request
//! takes a handle on it rather than a reference into the state. Locking empties
//! the cell: the `Arc` inside drops, and every key type wipes itself when the
//! last handle to it goes (spec: DK-7).
//!
//! That is what makes the lock safe to land at any moment. A request that
//! already took a handle finishes the work it began and releases it, and a
//! request that has not is refused before it does anything — which is DK-2's
//! "none of them partially succeeds" said about one operation rather than about
//! one connection.
//!
//! # When it happens
//!
//! When nobody wants the Library for long enough, which is [`lock_when_idle`]
//! (spec: DK-4), armed as serving starts and again at every unlock. There is no
//! lock to ask for: stopping the server already ends its hold on the keys, and a
//! lock on request would be a third way to arrive at a state two ways already
//! reach.
//! What counts as somebody being there is an authorized request that needs the
//! keys, and it is recorded where those are handed out —
//! `ServerState::unlocked`, the one door every piece of keyed work goes
//! through, so a route added later is counted by needing a key rather than by
//! being remembered in a list. It counts for as long as the work runs and not
//! for the moment it began: a [`KeyHandle`] marks somebody being here when it
//! is taken and again when it is let go, so an hour of packing a book is an
//! hour of the Library being wanted.
//!
//! The requests that need no key are deliberately silent: which Library this
//! is, and what this server is doing. The explorer asks the second of those
//! several times a second while a reader is open, and a tab left open is not a
//! person at the keyboard — a clock those requests kept moving would never
//! reach the end of an interval in exactly the case this exists for, somebody
//! who walked away mid-page.
//!
//! Silent about presence, and not about the state. The second of those two
//! carries which of DK-1's two states this device is in, because a lock nobody
//! asked for has nobody to answer: the window standing over that mid-page would
//! otherwise go on showing it until something it asked for was refused, which
//! for a reader whose pages nobody is turning is not soon. So it reads the state
//! out of the question it is already asking and gives the plaintext up. Reading
//! it takes no key and so is not activity — the interval runs out under the very
//! polling that reports it.
//!
//! How long "long enough" is is a policy parameter and never a constant of this
//! crate (spec: DK-4). The binary takes it from the command line, with the
//! environment behind that and a default behind both.
//!
//! # The way back
//!
//! The cell can be filled again, which is the unlock (spec: DK-1): the
//! Passphrase reopens the Library and `ServerState::unlock` hands what it
//! produced to [`Custody`], and the idle lock is armed afresh from that moment.
//!
//! What it never does is take the Passphrase itself. No route carries one: a
//! Passphrase typed into the explorer's page would be a Passphrase carried
//! through one, and that page is exactly what the key this server admits its
//! callers by is kept away from (spec: LA-3, LA-6). The Passphrase is taken from
//! a prompt that does not echo (spec: DK-10), and the one such prompt a running
//! server has is the desktop app's own window, not the explorer's page (spec:
//! DK-1), reached through an [`UnlockPrompt`]. What the explorer can do is ask
//! the server to wake it. A server started from the command line has no such
//! prompt — its terminal is not being read any more — so it is unlocked by
//! starting it again, which is what its refusal tells whoever meets it.

mod custody;
pub(crate) use custody::Custody;

mod idle;
pub(crate) use idle::Idle;

mod key_handle;
pub(crate) use key_handle::KeyHandle;

mod lock_when_idle;
pub use lock_when_idle::lock_when_idle;

mod unlock_prompt;
pub(crate) use unlock_prompt::Asked;
pub use unlock_prompt::UnlockPrompt;
