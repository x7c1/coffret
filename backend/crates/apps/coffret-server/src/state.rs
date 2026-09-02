use std::sync::Arc;

use coffret_device::{EntryFetches, OpenLibrary};
use tokio::time::Instant;

use crate::api_error::ApiError;
use crate::fill::Fills;
use crate::freeze::Freezes;
use crate::lock::{Custody, Idle, KeyHandle};
use crate::refresh::Refreshes;
use crate::sync::Syncs;

/// One Library, and what serving it needs beyond it.
///
/// The Library itself is not a field here. It is behind [`Custody`], and every
/// piece of work that needs it asks [`unlocked`](Self::unlocked) for a handle —
/// because the Passphrase was spent once, at startup, and the keys it produced
/// live from that unlock until a lock ends them (spec: DK-1). Emptying that cell
/// is the lock, and nothing else in this value can keep a key alive past one.
///
/// What is left beside the cell is either not the Library's secret or not the
/// Library at all. The name and the two identifying fields are what the status
/// bar shows, and they are held here rather than read through the cell so that a
/// locked server can still say which Library it is; the moment somebody was last
/// here is what decides when the cell is emptied without anybody asking
/// (spec: DK-4); and the five run-tracking values are this process's own account
/// of work in flight, gone when the process is, and never uploaded.
///
/// Nothing in this value ever leaves it — no key, no ciphertext, no token
/// reaches a response — and what a browser is answered with is drawn from it a
/// request at a time.
pub struct ServerState {
    /// What this device calls the Library, which is what was typed to start the
    /// server and what the status bar shows.
    ///
    /// It is this device's name for it rather than the Library's own: another
    /// device holding the same Library may call it something else (spec: CK-7).
    pub name: String,
    /// The Library this is (spec: FM-18), as the identity route spells it.
    library_id: String,
    /// Which provider the Library's Storage is, in the settings file's own word.
    ///
    /// The one thing about where a Library lives that a shell may show without
    /// reading the settings for itself: it names the provider and nothing about
    /// the account, the bucket, the folder, or the grant.
    provider: &'static str,
    /// The Library, open — until it is not.
    custody: Custody,
    /// When somebody was last here, which is what the idle lock measures
    /// (spec: DK-4).
    ///
    /// Shared rather than owned outright, because every handle this state hands
    /// out writes the end of its own span on it and outlives the borrow that
    /// took it.
    idle: Arc<Idle>,
    /// Who is already fetching which Entry, so two readers wanting one page
    /// fetch it once.
    pub fetches: EntryFetches,
    /// Which folder is being brought over in the background, and how far it has
    /// got.
    ///
    /// State of this process rather than of the Library, exactly as
    /// [`fetches`](Self::fetches) is: it is about work in flight here, it is
    /// gone when the process is, and nothing in it is ever uploaded.
    pub fills: Fills,
    /// Whether the mapped folders are being carried into the Library right now,
    /// and what the last run of that came to.
    ///
    /// The other half of [`fills`](Self::fills), going the other way, and device
    /// state in exactly the same sense. The two are separate because they are
    /// separate work over one Library and neither waits on the other: a folder
    /// being brought over and a dropped file being carried in can be happening at
    /// once, and a browser is told about both.
    pub syncs: Syncs,
    /// Which book is being packed into Packs right now, and what the last one
    /// came to.
    ///
    /// The third piece of background work, and device state in exactly the sense
    /// the other two are. It is apart from [`syncs`](Self::syncs) because it is
    /// the other way of carrying files in — one folder at a time, into Packs
    /// (spec: PK-7, PK-17), rather than the mappings entire one Container per
    /// file — and because a book being brought in must not be abandoned when
    /// something else is dropped.
    pub freezes: Freezes,
    /// Who is catching the catalog up with the Library right now.
    ///
    /// Unlike the three above it this holds no account of what happened: a
    /// refresh answers the request that asked for it, so there is nobody left to
    /// tell afterwards. What is kept is only that one is running, so a second
    /// caller waits rather than replaying the same records beside it.
    pub refreshes: Refreshes,
}

impl ServerState {
    /// Serves the Library that was opened, under the name it was opened by.
    pub fn new(name: String, library: OpenLibrary) -> Self {
        Self {
            name,
            library_id: library.library_id.to_hex(),
            provider: library.provider,
            custody: Custody::holding(library),
            idle: Arc::new(Idle::started()),
            fetches: EntryFetches::new(),
            fills: Fills::new(),
            syncs: Syncs::new(),
            freezes: Freezes::new(),
            refreshes: Refreshes::new(),
        }
    }

    /// The open Library, or the refusal a locked one owes every caller that
    /// needs a key (spec: DK-2).
    ///
    /// Asked once at the top of each piece of work and held for the whole of it.
    /// That is what makes an operation whole rather than half: whoever has a
    /// handle finishes with it however the lock lands, and whoever asks after
    /// the cell was emptied does nothing at all.
    ///
    /// And this is where somebody being here is recorded (spec: DK-4), because
    /// this is the one door every piece of work that needs the Library goes
    /// through. Presence is not "a request arrived" — the explorer asks what
    /// this server is doing several times a second while a reader is open, and
    /// an open tab is not a person at the keyboard. It is somebody wanting the
    /// Library itself: a page turned, a folder listed, a file dropped. A route
    /// added later inherits that by needing a key, and one that needs no key is
    /// silent here because it never asks.
    ///
    /// What is recorded is the whole span of that work and not the moment it
    /// began: the [`KeyHandle`] marks it at both ends and the stretch between
    /// them counts as well, so a piece of work that takes longer than the idle
    /// interval defers the lock rather than being shut out by it.
    pub(crate) fn unlocked(&self) -> Result<KeyHandle, ApiError> {
        let library = self.custody.unlocked().ok_or_else(ApiError::locked)?;
        Ok(KeyHandle::taken(library, Arc::clone(&self.idle)))
    }

    /// Locks the Library, and it is locked by the time this returns
    /// (spec: DK-3).
    ///
    /// `true` where this call is the one that locked it. A second lock is not a
    /// failure — what was asked for is a state, and the state is the same — so
    /// what the answer distinguishes is only which call is worth a line in the
    /// log.
    pub fn lock(&self) -> bool {
        self.custody.lock()
    }

    /// The Library this is, for the one route that answers while locked.
    pub(crate) fn library_id(&self) -> &str {
        self.library_id.as_str()
    }

    /// Which provider it is on, for the same route and the same reason.
    pub(crate) fn provider(&self) -> &'static str {
        self.provider
    }

    /// Records that somebody is here (spec: DK-4).
    ///
    /// Called once by the watcher as it starts — the moment this server begins
    /// serving, which is where the first interval is counted from. Every piece
    /// of work that needs the Library records itself instead, at both ends of
    /// its span, through the handle [`unlocked`](Self::unlocked) hands it.
    pub(crate) fn seen(&self) {
        self.idle.seen();
    }

    /// When that last was, which is now while a piece of work still holds the
    /// Library.
    pub(crate) fn last_seen(&self) -> Instant {
        self.idle.last_seen()
    }
}
