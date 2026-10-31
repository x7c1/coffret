use std::collections::BTreeSet;
use std::sync::Arc;

use coffret_device::{EntryPath, OpenLibrary, Redacted};
use tracing::{info, warn};

use crate::adds::AddId;
use crate::freeze::{freeze_dropped, Book};
use crate::state::ServerState;
use crate::sync::arm_sync;

use super::drop_reservation::DropReservation;
use super::release::release;

/// What a drop that was not stopped comes to once its last file is in, by
/// whichever route it arrived.
pub(super) struct FinishedDrop {
    /// Which route took it, for the event.
    pub(super) operation: &'static str,
    /// The add the drop is, which the run it arms carries (spec: LA-34).
    pub(super) add: AddId,
    /// The drop's reservation, where it asked for a Pack (spec: PK-17).
    pub(super) reservation: Option<DropReservation>,
    /// The Entry Paths that reservation names: the ones this drop added to
    /// it, and where it took over one a drop cut off left, the ones that drop
    /// had received.
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
/// Only where something landed, or where the drop took over a reservation a
/// drop cut off had left with files in it. A drop every file of which was
/// refused has otherwise left the folder exactly as it was, and a run over an
/// unchanged folder is a walk to find nothing; one that took over a cut-off
/// drop has finished receiving that drop's book all the same, and packs what
/// the earlier drop received. A drop stopped for the whole request is answered
/// where it was stopped and never reaches here.
pub(super) async fn finish_drop(
    state: &Arc<ServerState>,
    library: &OpenLibrary,
    finished: FinishedDrop,
) {
    let FinishedDrop {
        operation,
        add,
        reservation,
        reserved,
        written,
        refused,
        bytes,
        freeze,
    } = finished;
    let taken_over = reservation.as_ref().is_some_and(|held| held.taken_over);
    let packs = freeze && !reserved.is_empty() && (written > 0 || taken_over);
    if packs {
        // Receiving has ended, so the reservation names every page of the
        // book. It is marked received while it is still held live — no run
        // decides it before that — and stops being live before the run that
        // decides it is armed: a run of its own, never the one under way, which
        // read the reservations while this drop was still landing
        // (spec: PK-17).
        let held = reservation
            .as_ref()
            .map(|held| (held.id(), held.taken_over));
        if let Some(held) = reservation {
            mark_received(library, &held, operation).await;
        }
        state.adds.armed(&add, held, reserved.clone());
        freeze_dropped(Arc::clone(state), Book::dropped(reserved).carrying(add));
    } else if written == 0 {
        release(library, reservation, operation).await;
        state.adds.end(&add);
    } else {
        state.adds.armed(&add, None, BTreeSet::new());
        // A stop that came while the last file was landing is the sync's to
        // answer now, and a run under way is asked to stop for it too: it
        // may be about to read these files (see `stop_receiving`).
        let stopping = state.adds.stopping(&add);
        arm_sync(Arc::clone(state), Some(add));
        if stopping {
            state.syncs.ask_stop();
        }
    }

    // Counts and sizes, and which of the two flows was armed. No name of anything
    // reaches the event: the folder and the files are Entry Paths, which are the
    // user's own names for their own files (spec: EL-1).
    info!(
        operation,
        written, refused, bytes, freeze, taken_over, "files were added to a folder",
    );
}

/// Marks a finished drop's reservation received, so that the freeze about to
/// be armed may decide it (spec: PK-17).
///
/// A mark that fails is said and not raised, for the reason a release that
/// fails is: the drop has been taken, and the reservation it leaves is a drop
/// in progress — no freeze packs it, and the next drop of its folder takes it
/// over and packs the whole book. Said, because until then its files wait.
async fn mark_received(library: &OpenLibrary, held: &DropReservation, operation: &'static str) {
    let id = held.id();
    if let Err(refused) = library.mark_received(id).await {
        warn!(
            operation,
            reservation = %id,
            reason = %refused.redacted(),
            "the reservation of a finished drop could not be marked received; its files wait \
             for the folder to be dropped again",
        );
    }
}
