use std::collections::BTreeSet;
use std::sync::Arc;

use coffret_device::{EntryPath, LiveReservation, OpenLibrary};
use tracing::info;

use crate::freeze::{freeze_dropped, Book};
use crate::state::ServerState;
use crate::sync::arm_sync;

use super::release::release;

/// What a drop that was not stopped comes to once its last file is in, by
/// whichever route it arrived.
pub(super) struct FinishedDrop {
    /// Which route took it, for the event.
    pub(super) operation: &'static str,
    /// The drop's reservation, where it asked for a Pack (spec: PK-17).
    pub(super) reservation: Option<LiveReservation>,
    /// The Entry Paths it added to that reservation.
    pub(super) reserved: BTreeSet<EntryPath>,
    /// How many files landed.
    pub(super) written: usize,
    /// How many were refused.
    pub(super) refused: usize,
    /// How many bytes landed.
    pub(super) bytes: u64,
    /// Whether it asked for a Pack.
    pub(super) freeze: bool,
}

/// Arms the flow that carries a drop in, and says what it came to.
///
/// Only where something landed. A drop every file of which was refused has
/// left the folder exactly as it was, and a run over an unchanged folder is a
/// walk to find nothing. A drop stopped for the whole request is answered
/// where it was stopped and never reaches here.
pub(super) async fn finish_drop(
    state: &Arc<ServerState>,
    library: &OpenLibrary,
    finished: FinishedDrop,
) {
    let FinishedDrop {
        operation,
        reservation,
        reserved,
        written,
        refused,
        bytes,
        freeze,
    } = finished;
    if written == 0 {
        release(library, reservation, operation).await;
    } else if freeze {
        // Receiving has ended, so the reservation names every page this drop
        // landed and stops being live before the run that decides it is armed:
        // a run of its own, never the one under way, which read the
        // reservations while this drop was still landing (spec: PK-17).
        drop(reservation);
        freeze_dropped(Arc::clone(state), Book::dropped(reserved));
    } else {
        arm_sync(Arc::clone(state));
    }

    // Counts and sizes, and which of the two flows was armed. No name of anything
    // reaches the event: the folder and the files are Entry Paths, which are the
    // user's own names for their own files (spec: EL-1).
    info!(
        operation,
        written, refused, bytes, freeze, "files were added to a folder",
    );
}
