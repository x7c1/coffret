use std::sync::Arc;

use super::{LiveReservations, ReservationId};

/// One drop's hold on its reservation, from the moment it was made until the
/// drop stops receiving (spec: PK-17).
#[derive(Debug)]
pub struct LiveReservation {
    id: ReservationId,
    registry: Arc<LiveReservations>,
}

impl LiveReservation {
    /// The hold [`LiveReservations::hold`] hands out for `id`.
    pub(super) fn new(id: ReservationId, registry: Arc<LiveReservations>) -> Self {
        Self { id, registry }
    }

    /// The reservation held.
    pub fn id(&self) -> ReservationId {
        self.id
    }
}

impl Drop for LiveReservation {
    fn drop(&mut self) {
        self.registry.end(self.id);
    }
}
