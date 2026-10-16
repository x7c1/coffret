use std::sync::Arc;

use coffret_model::ContainerId;

use super::{FetchMemory, Flight, Turn};
use crate::fetch::parcel_read::front::FrontRefusal;

/// What one read of a Container's front came to: the bytes as they arrived,
/// not yet opened.
pub(crate) type FrontRead = Result<Arc<[u8]>, FrontRefusal>;

impl FetchMemory {
    /// The front of one Container as it arrived, where it is kept.
    pub(crate) fn front(&self, container_id: ContainerId) -> Option<Arc<[u8]>> {
        self.locked().fronts.get(container_id)
    }

    /// The same, or a wait on the read of it under way, or the read to make.
    pub(crate) fn front_of(
        &self,
        container_id: ContainerId,
    ) -> Turn<'_, Arc<[u8]>, ContainerId, FrontRead> {
        let mut kept = self.locked();
        if let Some(front) = kept.fronts.get(container_id) {
            return Turn::Kept(front);
        }
        if let Some(waiter) = kept.front_reads.join(&container_id) {
            return Turn::Join(waiter);
        }
        Turn::Read(Flight::start(
            self,
            &mut kept,
            |kept| &mut kept.front_reads,
            container_id,
        ))
    }

    /// Keeps the front of one Container, making room by forgetting the oldest.
    pub(crate) fn remember_front(&self, container_id: ContainerId, front: Arc<[u8]>) {
        self.locked().fronts.keep(container_id, front);
    }
}
