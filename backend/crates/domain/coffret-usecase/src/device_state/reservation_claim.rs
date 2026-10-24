use tokio::sync::MutexGuard;

/// One drop's turn at choosing its reservation, from
/// [`LiveReservations::claim`](super::LiveReservations::claim) until it drops.
#[derive(Debug)]
pub struct ReservationClaim<'a> {
    _claimed: MutexGuard<'a, ()>,
}

impl<'a> ReservationClaim<'a> {
    /// The turn [`LiveReservations::claim`](super::LiveReservations::claim)
    /// hands out once it has taken `claimed`.
    pub(super) fn new(claimed: MutexGuard<'a, ()>) -> Self {
        Self { _claimed: claimed }
    }
}
