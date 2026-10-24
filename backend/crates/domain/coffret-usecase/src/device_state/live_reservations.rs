use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::{LiveReservation, ReservationClaim, ReservationId};

/// The reservations a drop in this process is still writing into, and every
/// one that ever was (spec: PK-17).
///
/// A reservation is *live* from the moment the drop that made it holds it to
/// the moment the drop stops receiving: until then it names only the files
/// that have landed so far, and a freeze that took it for the whole book would
/// release it — with a commit, or as lapsed — while pages are still to come, and
/// those pages would land with nothing keeping a sync from carrying them in
/// one Container each. So no run decides a live reservation; the drop's own
/// run, armed once it stops receiving, is what decides it.
///
/// Held in memory and nowhere else, like the scratch names a writer holds:
/// nothing here is ever written to the Index, and a process started afterwards
/// holds nothing live. What tells it which of the reservations it finds a run
/// may decide is the Index's own mark of a drop that finished receiving
/// ([`Reservation::received`](super::Reservation::received)): one without it
/// was cut off and waits for its folder to be dropped again (spec: LA-12). It
/// belongs to whoever opens the Library rather than to one open handle, so a
/// server that is locked and unlocked again while a drop is still landing on
/// the old handle goes on knowing that drop is live.
#[derive(Debug, Default)]
pub struct LiveReservations {
    held: Mutex<Held>,
    /// Taken by a drop for the whole of choosing its reservation — finding one
    /// to take over, or making one and holding it (see [`claim`](Self::claim)).
    claiming: tokio::sync::Mutex<()>,
}

#[derive(Debug, Default)]
struct Held {
    /// The reservations a drop holds now.
    live: BTreeSet<ReservationId>,
    /// Every reservation a drop in this process has held, live or not.
    ///
    /// What tells a run that a reservation became live after it read the
    /// reservations, and so may have gained paths it never read
    /// ([`LiveSince`]). One number per drop the process took.
    ever: BTreeSet<ReservationId>,
}

/// Nothing live: what a run is told where nobody in its process takes drops,
/// the command line's run for one.
pub(crate) static NONE_LIVE: LiveReservations = LiveReservations::new();

impl LiveReservations {
    /// Nothing held yet.
    pub const fn new() -> Self {
        Self {
            held: Mutex::new(Held {
                live: BTreeSet::new(),
                ever: BTreeSet::new(),
            }),
            claiming: tokio::sync::Mutex::const_new(()),
        }
    }

    /// Waits until no other drop in this process is choosing its reservation,
    /// and keeps every other one waiting until the value returned drops
    /// (spec: PK-17).
    ///
    /// What makes a drop's choice one step: a reservation is made in the Index
    /// before it can be held here, and in between it is one in progress that
    /// nobody holds — exactly what a second drop of the same folder takes over.
    /// So the drop making one and the drop looking for one to take over each
    /// claim first, and the reservation one of them made is held before the
    /// other looks: exactly one of them holds it.
    pub async fn claim(&self) -> ReservationClaim<'_> {
        ReservationClaim::new(self.claiming.lock().await)
    }

    /// Holds `id` live from now until the value returned drops.
    pub fn hold(self: &Arc<Self>, id: ReservationId) -> LiveReservation {
        let mut held = self.held();
        held.live.insert(id);
        held.ever.insert(id);
        LiveReservation::new(id, Arc::clone(self))
    }

    /// Holds `id` live as [`hold`](Self::hold) does, unless a drop in this
    /// process holds it already, in which case `None`.
    ///
    /// What a drop taking over a reservation it found in progress asks
    /// (spec: PK-17): two drops of one folder arriving together must not both
    /// take the same one over, and the question and the hold are one step
    /// under the lock, so only one of them is answered with it.
    pub fn try_hold(self: &Arc<Self>, id: ReservationId) -> Option<LiveReservation> {
        let mut held = self.held();
        if !held.live.insert(id) {
            return None;
        }
        held.ever.insert(id);
        Some(LiveReservation::new(id, Arc::clone(self)))
    }

    /// Whether a drop in this process holds `id` now.
    pub fn is_live(&self, id: ReservationId) -> bool {
        self.held().live.contains(&id)
    }

    /// What a run reading the reservations now is to spare until it decides
    /// any (see [`LiveSince::spares`]).
    pub(crate) fn since_now(&self) -> LiveSince<'_> {
        let held = self.held();
        LiveSince {
            registry: self,
            ended: held.ever.difference(&held.live).copied().collect(),
        }
    }

    /// Ends the hold on `id`, as its [`LiveReservation`] drops.
    pub(super) fn end(&self, id: ReservationId) {
        self.held().live.remove(&id);
    }

    /// A poisoned lock is taken all the same: each change to the sets is one
    /// insertion or one removal, so a panic elsewhere cannot leave them
    /// half-written.
    fn held(&self) -> MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The live reservations as one run sees them, from the moment it reads the
/// reservations to the moment it decides any.
///
/// A run reads the reservations before its scan and decides them after it, and
/// a drop can start, land pages and stop in between. So what the run spares is
/// not only what is live when it decides: a reservation that was live when it
/// read them may have gained paths since, and one that became live after may
/// have been read before its later pages were added. Only a reservation that
/// was never live in this process, or had already stopped being live when the
/// run read it, is one whose paths the run read in full.
pub(crate) struct LiveSince<'a> {
    registry: &'a LiveReservations,
    /// The reservations that had been live and were no longer when the run
    /// read them.
    ended: BTreeSet<ReservationId>,
}

impl LiveSince<'_> {
    /// Whether the run must leave `id` alone: it is live now, or was at some
    /// moment since the run read the reservations.
    pub(crate) fn spares(&self, id: ReservationId) -> bool {
        let held = self.registry.held();
        held.live.contains(&id) || (held.ever.contains(&id) && !self.ended.contains(&id))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::LiveReservations;
    use crate::device_state::ReservationId;

    #[test]
    fn a_reservation_held_already_is_not_held_twice() {
        let registry = Arc::new(LiveReservations::new());
        let id = ReservationId::new(1);
        let hold = registry.try_hold(id).expect("nothing holds it yet");
        assert!(registry.try_hold(id).is_none(), "a drop holds it");
        drop(hold);
        assert!(
            registry.try_hold(id).is_some(),
            "and once it lets go, it may be held again"
        );
    }

    #[test]
    fn a_reservation_is_live_for_as_long_as_its_hold() {
        let registry = Arc::new(LiveReservations::new());
        let id = ReservationId::new(1);
        let hold = registry.hold(id);
        assert!(registry.is_live(id));
        drop(hold);
        assert!(!registry.is_live(id));
    }

    // PK-17: a run spares what was live at any moment since it read the
    // reservations, and nothing that had stopped before or was never held.
    #[test]
    fn a_run_spares_what_was_live_at_any_moment_since_it_read() {
        let registry = Arc::new(LiveReservations::new());
        let before = ReservationId::new(1);
        let during = ReservationId::new(2);
        let started = ReservationId::new(3);
        let held_through = ReservationId::new(4);
        let never = ReservationId::new(5);

        drop(registry.hold(before));
        let during_hold = registry.hold(during);
        let through_hold = registry.hold(held_through);
        let since = registry.since_now();
        drop(during_hold);
        drop(registry.hold(started));

        assert!(!since.spares(before), "it stopped before the run read it");
        assert!(since.spares(during), "it may have gained paths since");
        assert!(since.spares(started), "it was read before its later pages");
        assert!(since.spares(held_through), "it is live now");
        assert!(!since.spares(never), "nothing in this process held it");
        drop(through_hold);
    }
}
