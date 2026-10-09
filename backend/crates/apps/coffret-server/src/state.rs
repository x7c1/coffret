use std::sync::Arc;

use anyhow::ensure;
use coffret_device::{EntryFetches, OpenLibrary};
use tokio::sync::watch;
use tokio::time::Instant;

use crate::allowance::Allowance;
use crate::api_error::{ApiError, WayBack};
use crate::delete::Deletes;
use crate::fill::Fills;
use crate::freeze::Freezes;
use crate::lock::{Asked, Custody, Idle, KeyHandle, UnlockPrompt};
use crate::pending_rows::PendingRows;
use crate::reconnect::{Consent, DriveConsent, Reconnects};
use crate::refresh::{Catalog, Refreshes};
use crate::server_id::ServerId;
use crate::sync::Syncs;
use crate::unlocked::Unlocked;

/// One Library, and what serving it needs beyond it.
///
/// The Library itself is not a field here. It is behind a `Custody` cell, and
/// every piece of work that needs it asks this type's own `unlocked` for a
/// handle — both of them this crate's and neither of them exported, because
/// holding the Library is not something a caller outside here reaches into.
/// The Passphrase was spent at startup, and the keys it produced live from that
/// unlock until a lock ends them (spec: DK-1). Emptying that cell is the lock,
/// and nothing else in this value can keep a key alive past one; filling it
/// again with what the Passphrase reopened is the unlock, which only the
/// process the server runs in can do (see [`unlock`](Self::unlock)).
///
/// What is left beside the cell is either not the Library's secret or not the
/// Library at all. The name and the two identifying fields are what the status
/// bar shows, and they are held here rather than read through the cell so that a
/// locked server can still say which Library it is; the moment somebody was last
/// here is what decides when the cell is emptied without anybody asking
/// (spec: DK-4); and the run-tracking values are this process's own account of
/// work in flight — and of how far this device has got with the Library — gone
/// when the process is, and never uploaded (spec: LA-12).
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
    /// What this process calls itself, which every answer about work in flight
    /// carries.
    ///
    /// Not about the Library and not about this device: it is drawn from
    /// entropy as this value is built, and what it is for is telling one
    /// process's answers from the next's. A browser remembers things that are
    /// true only of one process — which run's line somebody read and put away,
    /// counted from 1 by each of the three flows — and a restart is an ordinary
    /// step here, since a server started from the command line is unlocked by
    /// starting it again (spec: DK-1). An unlock in place keeps it: the process
    /// and its runs are the same ones. See [`ServerId`] for why it is this value
    /// and not one that was already lying around.
    server: ServerId,
    /// Which provider the Library's Storage is, in the settings file's own word.
    ///
    /// The one thing about where a Library lives that a shell may show without
    /// reading the settings for itself: it names the provider and nothing about
    /// the account, the bucket, the folder, or the grant.
    provider: &'static str,
    /// The Library, open — until it is not, and then open again where the
    /// process this server runs in can take the Passphrase.
    custody: Custody,
    /// Said whenever the cell is filled again, so the idle lock waiting on a
    /// locked Library hears the unlock and is armed afresh from it.
    ///
    /// A `watch` rather than a notification, because what the waiter needs is
    /// never to miss one that landed between its last look and its wait: it
    /// subscribes, looks at the cell, and only then waits.
    unlocks: watch::Sender<()>,
    /// What wakes the desktop app's Passphrase window, where this server runs
    /// inside the app, and `None` for one started from the command line.
    prompt: Option<UnlockPrompt>,
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
    /// What is being deleted from the Library right now, and what the last
    /// deletion came to.
    ///
    /// The fourth piece of background work, and device state in the sense the
    /// other three are. Apart from them because a deletion that rebuilds a Pack
    /// reads it whole and uploads its replacement (spec: PK-10), which is no
    /// shorter than packing a book.
    pub deletes: Deletes,
    /// The one turn a sync, a freeze and a deletion take at this device's
    /// pending rows, in the order they ask for it.
    ///
    /// The three own those rows for the whole of a run and a second owner is
    /// refused rather than made to wait (spec: OC-2), so the three workers above
    /// queue here rather than refusing each other. See [`PendingRows`].
    pub pending_rows: PendingRows,
    /// What one request may bring, and how the room to take it is asked after.
    ///
    /// Here rather than in the Library, which puts no number on a file: these
    /// are one HTTP server's own budgets on what it reads off a socket in one go
    /// (spec: LA-9, LA-11, and [`Allowance`](crate::Allowance) for why those
    /// numbers). The router mounts the upload route with the first of them and
    /// the route itself keeps the rest.
    pub allowance: Allowance,
    /// How this device's catalog stands with the Library, and what stopped the
    /// last attempt to catch it up where one did.
    ///
    /// Device state in the sense the three above are — it is about this process
    /// and nothing in it is ever uploaded — and the one piece of it that is not
    /// about work somebody set going: it is what the server did to itself before
    /// it answered anything. A browser needs it because the alternative is
    /// reading an empty listing as an empty Library.
    pub catalog: Catalog,
    /// Who is catching the catalog up with the Library right now.
    ///
    /// Unlike the three above it this holds no account of what happened: a
    /// refresh answers the request that asked for it, so there is nobody left to
    /// tell afterwards. What is kept is only that one is running, so a second
    /// caller waits rather than replaying the same records beside it.
    pub refreshes: Refreshes,
    /// Whether a consent flow is waiting to renew the grant Storage is reached
    /// through, and what the last one came to.
    ///
    /// Device state in the sense the run-tracking values above are: about this
    /// process, and never uploaded. Nothing in it is a credential — the grant
    /// itself goes into the account's cache and nowhere else (spec: SA-6).
    pub reconnects: Reconnects,
    /// How a consent flow is asked for, which is Google's own page everywhere
    /// but in a case.
    pub(crate) consent: Arc<dyn Consent>,
}

impl ServerState {
    /// Serves the Library that was opened, under the name it was opened by.
    pub fn new(name: String, library: OpenLibrary) -> Self {
        Self {
            name,
            library_id: library.library_id.to_hex(),
            server: ServerId::drawn(),
            provider: library.provider,
            custody: Custody::holding(library),
            unlocks: watch::Sender::new(()),
            prompt: None,
            idle: Arc::new(Idle::started()),
            fetches: EntryFetches::new(),
            fills: Fills::new(),
            syncs: Syncs::new(),
            freezes: Freezes::new(),
            deletes: Deletes::new(),
            pending_rows: PendingRows::new(),
            allowance: Allowance::generous(),
            catalog: Catalog::new(),
            refreshes: Refreshes::new(),
            reconnects: Reconnects::new(),
            consent: Arc::new(DriveConsent),
        }
    }

    /// Serves the same Library, asking for consent through `consent`.
    ///
    /// The binary never calls it, for the reason it never calls
    /// [`within`](Self::within): what it ships asks through Google's own page
    /// ([`DriveConsent`]). It exists so a case can drive a reconnect to its end
    /// without a browser, a person, or Google's token endpoint.
    pub fn consenting_through(mut self, consent: Arc<dyn Consent>) -> Self {
        self.consent = consent;
        self
    }

    /// Serves the same Library, asking the process it runs in for the
    /// Passphrase through `prompt` once it has locked (spec: DK-1).
    ///
    /// What the desktop app's server is built with, through
    /// [`Launch`](crate::Launch). Without it the server is the command line's,
    /// whose locked refusal says to start it again.
    pub fn prompting_through(mut self, prompt: UnlockPrompt) -> Self {
        self.prompt = Some(prompt);
        self
    }

    /// Serves the same Library within a different allowance.
    ///
    /// The binary never calls it: what it serves within is the one
    /// [`Allowance::generous`] states, and a server whose budgets came from
    /// somewhere else would be a server nobody could reason about from the
    /// constants. It exists so a case can reach a budget at all, which is the
    /// reason [`Allowance`] is a value rather than three constants.
    pub fn within(mut self, allowance: Allowance) -> Self {
        self.allowance = allowance;
        self
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
        let library = self
            .custody
            .unlocked()
            .ok_or_else(|| ApiError::locked(self.way_back()))?;
        Ok(KeyHandle::taken(library, Arc::clone(&self.idle)))
    }

    /// Locks the Library, and it is locked by the time this returns — the move
    /// the idle interval running out makes (spec: DK-4).
    pub fn lock(&self) {
        self.custody.lock();
    }

    /// Holds the Library open again, with what the Passphrase has just
    /// reopened — the move a lock is the inverse of (spec: DK-1).
    ///
    /// Only the process this server runs in calls it, with a Library it opened
    /// from a Passphrase taken in a prompt of its own (spec: DK-10): no route
    /// carries a Passphrase, and none reaches here.
    ///
    /// A Library already open is left as it is and what was handed in is
    /// dropped at once, which wipes its keys (spec: DK-7): two unlocks under way
    /// together end with one set of keys, the first to arrive. A Library that
    /// is not the one this server serves is refused, whatever it is called on
    /// this device — the name a Passphrase reopened could have been made to
    /// point somewhere else since the server started, and serving that under
    /// this server's identity would be answering for one Library with another.
    ///
    /// Once it has taken the Library the idle lock is armed afresh, so the
    /// interval is counted from this moment (spec: DK-4).
    pub fn unlock(&self, library: OpenLibrary) -> anyhow::Result<Unlocked> {
        ensure!(
            library.library_id.to_hex() == self.library_id,
            "the Library called {:?} on this device is no longer the one this server was \
             started for; quit the app and open it again",
            self.name,
        );
        if !self.custody.unlock(library) {
            return Ok(Unlocked::Already);
        }
        self.unlocks.send_replace(());
        Ok(Unlocked::Now)
    }

    /// Waits until the Library is held open again, and returns at once where
    /// it already is.
    pub(crate) async fn until_unlocked(&self) {
        // Subscribed before the cell is looked at, so that an unlock landing
        // between the look and the wait is one the wait has already been told
        // of.
        let mut unlocks = self.unlocks.subscribe();
        while !self.holds_library() {
            // The sender is this value's own and lives as long as it does, so
            // it cannot have gone while this borrows it.
            if unlocks.changed().await.is_err() {
                return;
            }
        }
    }

    /// Asks the process this server runs in to take the Passphrase again, and
    /// says whether anybody was there to ask.
    pub(crate) fn ask_for_passphrase(&self) -> Asked {
        self.prompt
            .as_ref()
            .map_or(Asked::Unheard, UnlockPrompt::ask)
    }

    /// Where a locked server takes the Passphrase again, which is what its
    /// refusal tells the owner.
    pub(crate) fn way_back(&self) -> WayBack {
        match self.prompt {
            Some(_) => WayBack::InTheApp,
            None => WayBack::ByStartingAgain,
        }
    }

    /// Whether this device still holds the Library open (spec: DK-1).
    ///
    /// It takes no key handle, which is the point of it: this records no
    /// presence and defers no lock (spec: DK-4). Public for the desktop app,
    /// which asks it before offering a Passphrase window nobody needs.
    pub fn holds_library(&self) -> bool {
        self.custody.holds()
    }

    /// The Library this is, for the one route that answers while locked.
    pub(crate) fn library_id(&self) -> &str {
        self.library_id.as_str()
    }

    /// Which provider it is on, for the same route and the same reason.
    pub(crate) fn provider(&self) -> &'static str {
        self.provider
    }

    /// What this process calls itself, for the answers that carry it.
    pub(crate) fn server(&self) -> &str {
        self.server.as_str()
    }

    /// Records that somebody is here (spec: DK-4).
    ///
    /// Called by the watcher each time it is armed — the moment this server
    /// begins serving, and the moment of every unlock after a lock — which is
    /// where each interval is counted from. Every piece of work that needs the
    /// Library records itself instead, at both ends of its span, through the
    /// handle [`unlocked`](Self::unlocked) hands it.
    pub(crate) fn seen(&self) {
        self.idle.seen();
    }

    /// When that last was, which is now while a piece of work still holds the
    /// Library.
    pub(crate) fn last_seen(&self) -> Instant {
        self.idle.last_seen()
    }
}
