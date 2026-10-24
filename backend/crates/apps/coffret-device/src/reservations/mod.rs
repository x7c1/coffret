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
/// are called in: [`take_over`](OpenLibrary::take_over) or else
/// [`reserve`](OpenLibrary::reserve) before its first file lands,
/// [`extend_reservation`](OpenLibrary::extend_reservation) before each one
/// does, and [`mark_received`](OpenLibrary::mark_received) once it has stopped
/// receiving and arms its freeze — or
/// [`release_reservation`](OpenLibrary::release_reservation) where the drop is
/// refused and nothing is armed for it. A freeze that packs the
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
    /// decides the reservation, and until the drop marks it received none packs
    /// a page of it either.
    ///
    /// The reservation is made and held in one step, under the
    /// [`claim`](coffret_usecase::device_state::LiveReservations::claim)
    /// [`take_over`](Self::take_over) waits on too: between the Index making it
    /// and this process holding it, it is a drop in progress nobody holds, and a
    /// second drop of the folder looking in that moment would take it over.
    pub async fn reserve(&self, folder: Option<&EntryPath>) -> Result<LiveReservation> {
        let _claim = self.live.claim().await;
        let id = self.index.reserve(folder).await?;
        Ok(self.live.hold(id))
    }

    /// Takes over a drop in progress onto `folder` that no drop in this process
    /// holds — one cut off by the process that took it stopping — and holds it
    /// live as [`reserve`](Self::reserve) holds a new one, answering with the
    /// paths it already names; `None` where there is none (spec: PK-17, LA-12).
    ///
    /// What lets a book whose drop was cut off come to one Pack: the same
    /// folder dropped again goes on adding to the reservation the first drop
    /// left, rather than making a second one beside it, and the freeze it arms
    /// once it finishes receiving packs both drops' files together.
    ///
    /// Where several were cut off on the folder, the earliest made is taken
    /// over, and each later one is folded into it: its paths are added to the
    /// earliest, and then it is released. So the paths answered are every
    /// cut-off drop's, and the folder is left with one reservation in
    /// progress rather than one per drop that was cut off. A process stopping
    /// part way through the folding leaves a path named twice, which the next
    /// drop folds again. A reservation a drop in this process still holds is
    /// neither taken over nor folded: it is that drop's.
    ///
    /// Under the same claim as [`reserve`](Self::reserve), so a reservation
    /// another drop has just made is already held when this looks.
    pub async fn take_over(
        &self,
        folder: Option<&EntryPath>,
    ) -> Result<Option<(LiveReservation, BTreeSet<EntryPath>)>> {
        let _claim = self.live.claim().await;
        let mut taken: Option<(LiveReservation, BTreeSet<EntryPath>)> = None;
        for reservation in self.reservations().await? {
            if !reservation.in_progress() || reservation.folder.as_ref() != folder {
                continue;
            }
            let Some(held) = self.live.try_hold(reservation.id) else {
                continue;
            };
            match &mut taken {
                None => taken = Some((held, reservation.paths)),
                Some((earliest, paths)) => {
                    for path in &reservation.paths {
                        self.index.extend_reservation(earliest.id(), path).await?;
                    }
                    self.index.release_reservation(held.id()).await?;
                    paths.extend(reservation.paths);
                }
            }
        }
        Ok(taken)
    }

    /// Adds `path` to the reservation `id`, before the file the drop is writing
    /// there lands (spec: PK-17).
    pub async fn extend_reservation(&self, id: ReservationId, path: &EntryPath) -> Result<()> {
        Ok(self.index.extend_reservation(id, path).await?)
    }

    /// Marks the reservation `id` received, its drop having stopped receiving
    /// with every file it brought: from then on a freeze may decide it, and a
    /// server started afterwards takes it up (spec: PK-17, LA-12).
    pub async fn mark_received(&self, id: ReservationId) -> Result<()> {
        Ok(self.index.mark_received(id).await?)
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

#[cfg(test)]
mod tests;
