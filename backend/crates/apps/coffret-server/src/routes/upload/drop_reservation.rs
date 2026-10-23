use std::collections::BTreeSet;

use coffret_device::{EntryPath, LiveReservation, OpenLibrary, ReservationId};
use tracing::info;

use crate::api_error::ApiError;

/// A drop's reservation, held live for as long as the drop is receiving
/// (spec: PK-17).
pub(super) struct DropReservation {
    /// The hold on it, which keeps every freeze in this process from deciding
    /// it while pages are still to come.
    pub(super) live: LiveReservation,
    /// Whether the drop took over the reservation a drop cut off part way left
    /// on its folder, rather than making one of its own.
    pub(super) taken_over: bool,
}

impl DropReservation {
    /// The reservation a drop onto `folder` that asked for a Pack adds its
    /// files to, made before its first file lands, and the paths it names
    /// already (spec: PK-17, LA-12).
    ///
    /// Where an earlier drop onto the same folder was cut off before it
    /// finished receiving, this drop takes its reservation over
    /// ([`OpenLibrary::take_over`]) and the paths that drop received are this
    /// one's from the start. Anywhere else it is a new reservation naming no
    /// path.
    ///
    /// The event says a cut-off drop was taken over and how many files it had
    /// received: counts only, the folder and the files being Entry Paths
    /// (spec: EL-1).
    pub(super) async fn hold(
        library: &OpenLibrary,
        folder: Option<&EntryPath>,
        operation: &'static str,
    ) -> Result<(Self, BTreeSet<EntryPath>), ApiError> {
        if let Some((live, received)) = library.take_over(folder).await? {
            info!(
                operation,
                received = received.len(),
                "a drop cut off part way was taken over by a drop of its folder",
            );
            return Ok((
                Self {
                    live,
                    taken_over: true,
                },
                received,
            ));
        }
        let live = library.reserve(folder).await?;
        Ok((
            Self {
                live,
                taken_over: false,
            },
            BTreeSet::new(),
        ))
    }

    /// The reservation held.
    pub(super) fn id(&self) -> ReservationId {
        self.live.id()
    }
}
