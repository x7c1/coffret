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
//! That is what makes the lock safe to ask for at any moment. A request that
//! already took a handle finishes the work it began and releases it, and a
//! request that has not is refused before it does anything — which is DK-3's
//! "has taken effect by the time it returns" said about a server answering many
//! callers at once, and DK-2's "none of them partially succeeds" said about one
//! operation rather than about one connection.
//!
//! # Two ways it happens
//!
//! Somebody asks, through `POST /api/lock`. Or nobody wants the Library for
//! long enough, which is [`lock_when_idle`] (spec: DK-4). What counts as
//! somebody being there is an authorized request that needs the keys, and it is
//! recorded where those are handed out — `ServerState::unlocked`, the one door
//! every piece of keyed work goes through, so a route added later is counted by
//! needing a key rather than by being remembered in a list. It counts for as
//! long as the work runs and not for the moment it began: a [`KeyHandle`] marks
//! somebody being here when it is taken and again when it is let go, so an hour
//! of packing a book is an hour of the Library being wanted.
//!
//! The requests that need no key are deliberately silent: which Library this is,
//! what this server is doing, and the lock itself. The explorer asks the second
//! of those several times a second while a reader is open, and a tab left open
//! is not a person at the keyboard — a clock those requests kept moving would
//! never reach the end of an interval in exactly the case this exists for,
//! somebody who walked away mid-page.
//!
//! How long "long enough" is is a policy parameter and never a constant of this
//! crate (spec: DK-4). The binary takes it from the command line, with the
//! environment behind that and a default behind both.
//!
//! # What it does not do
//!
//! It does not unlock. The Passphrase opens a Library and the Passphrase is
//! typed at a terminal, so a locked server is unlocked by starting it again —
//! which is what the refusal tells whoever meets it. An unlock route would
//! carry the Passphrase through a browser, and that is a boundary this product
//! has not crossed.

mod custody;
pub(crate) use custody::Custody;

mod idle;
pub(crate) use idle::Idle;

mod key_handle;
pub(crate) use key_handle::KeyHandle;

mod lock_when_idle;
pub use lock_when_idle::lock_when_idle;
