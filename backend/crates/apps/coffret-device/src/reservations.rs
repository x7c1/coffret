use std::collections::BTreeSet;

use coffret_model::EntryPath;
use coffret_usecase::device_state::{LiveReservation, Reservation, ReservationId};

use crate::error::Result;
use crate::open_library::OpenLibrary;

/// A drop's choice to add its files as a Pack, kept on this device so that it
/// outlives the process that took the drop (spec: PK-17, LA-12).
///
/// The Index holds it, beside the mappings and the pending rows, and nothing of
/// it is ever uploaded (spec: CK-7). What a drop does with it is the order these
/// are called in: [`reserve`](OpenLibrary::reserve) before its first file
/// lands, [`extend_reservation`](OpenLibrary::extend_reservation) before each
/// one does, and [`release_reservation`](OpenLibrary::release_reservation) where
/// the drop is refused and nothing is armed for it. A freeze that packs the
/// files releases the reservation with its commit, and one that finds none of
/// them eligible releases it on its own (see
/// [`freeze`](OpenLibrary::freeze)).
impl OpenLibrary {
    /// Records that a drop onto `folder` is to be added as a Pack, before any
    /// of its files has landed, and holds the reservation live until the value
    /// returned drops (spec: PK-17).
    ///
    /// The drop keeps it for as long as it is receiving and lets it go before
    /// it arms its own freeze: while it is held, no freeze in this process
    /// decides the reservation, whatever it packs of the pages landed so far.
    pub async fn reserve(&self, folder: Option<&EntryPath>) -> Result<LiveReservation> {
        let id = self.index.reserve(folder).await?;
        Ok(self.live.hold(id))
    }

    /// Adds `path` to the reservation `id`, before the file the drop is writing
    /// there lands (spec: PK-17).
    pub async fn extend_reservation(&self, id: ReservationId, path: &EntryPath) -> Result<()> {
        Ok(self.index.extend_reservation(id, path).await?)
    }

    /// Releases the reservation `id`, its drop having been refused (spec: PK-17).
    pub async fn release_reservation(&self, id: ReservationId) -> Result<()> {
        Ok(self.index.release_reservation(id).await?)
    }

    /// Every reservation this device holds, in the order they were made — the
    /// order a server takes them up in after it starts (spec: PK-17, LA-12).
    pub async fn reservations(&self) -> Result<Vec<Reservation>> {
        Ok(self.index.reservations().await?)
    }

    /// Every Entry Path a reservation of this device names: the files waiting
    /// for their Pack (spec: PK-17).
    pub async fn reserved_paths(&self) -> Result<BTreeSet<EntryPath>> {
        Ok(self
            .reservations()
            .await?
            .into_iter()
            .flat_map(|reservation| reservation.paths)
            .collect())
    }
}
