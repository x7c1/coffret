use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::time::Instant;

/// When somebody was last here, and whether they still are.
///
/// One moment and a count of the spans still open, written wherever the open
/// Library is taken hold of and let go of, and read by the one task that watches
/// it (spec: DK-4). What counts as somebody being here is settled at the one
/// place that hands those holds out, `ServerState::unlocked`, and not in this
/// cell.
///
/// The clock is [`tokio::time::Instant`] rather than the standard library's, so
/// that a case can state a quarter of an hour of quiet instead of spending one.
pub(crate) struct Idle {
    presence: Mutex<Presence>,
}

/// The two things read together, so that neither is read against a stale other.
struct Presence {
    /// When a hold was last taken or let go.
    at: Instant,
    /// How many holds on the Library are open right now.
    held: usize,
}

impl Idle {
    /// Counts from now, until the watcher starts and counts from itself.
    ///
    /// The moment the Library was opened is a placeholder rather than the start
    /// of the first interval: opening a Library and catching its catalog up both
    /// happen before anything is served, and neither is time anybody could have
    /// been here for. [`lock_when_idle`](super::lock_when_idle) marks the real
    /// start as it begins to watch.
    pub(crate) fn started() -> Self {
        Self {
            presence: Mutex::new(Presence {
                at: Instant::now(),
                held: 0,
            }),
        }
    }

    /// Records that somebody is here.
    pub(crate) fn seen(&self) {
        self.presence().at = Instant::now();
    }

    /// Records a hold being taken, which opens a span of somebody being here.
    pub(crate) fn taken(&self) {
        let mut presence = self.presence();
        presence.at = Instant::now();
        presence.held += 1;
    }

    /// Records it being let go, which closes that span at now.
    ///
    /// Every hold that was taken is let go exactly once — a
    /// [`KeyHandle`](super::KeyHandle) is what takes one, and dropping it is
    /// what lets it go — so the count comes back down to nobody being here.
    pub(crate) fn released(&self) {
        let mut presence = self.presence();
        presence.at = Instant::now();
        presence.held -= 1;
    }

    /// When that last was, which is now while anybody still holds the Library.
    ///
    /// A piece of work that outlasts the idle interval is not an idle server: it
    /// is the busiest the server gets. So while a hold is open there is no
    /// quiet to measure, and the interval is counted afresh from the moment the
    /// last of them was let go. The cost of saying it this way is that work
    /// which never finishes keeps a server unlocked, which is the same thing as
    /// saying it is still working.
    pub(crate) fn last_seen(&self) -> Instant {
        let presence = self.presence();
        if presence.held > 0 {
            Instant::now()
        } else {
            presence.at
        }
    }

    /// The two of them, whatever a guard was left holding.
    ///
    /// Poisoning is recovered from for the reason [`Custody`](super::Custody)
    /// recovers from it: only a panic under this guard could poison it and
    /// there is nothing here to panic — a read of an `Instant`, a write of one,
    /// and a count moving by one. Recovering says that plainly, and says what it
    /// would cost to be wrong: a server that stopped being able to read its own
    /// clock would lock itself over a failure that had nothing to do with
    /// anybody being here.
    fn presence(&self) -> MutexGuard<'_, Presence> {
        self.presence.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
