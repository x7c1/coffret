use std::sync::Arc;

use coffret_device::Redacted;
use tracing::{info, warn};

use crate::state::ServerState;

use super::{freeze_folder, Book};

/// Takes up every drop this device recorded as waiting for its Pack that
/// finished receiving, in the order the drops were made, and leaves the ones
/// cut off part way for their folder to be dropped again (spec: PK-17, LA-12).
///
/// What a server holds about its work lives as long as the process, so a
/// freeze a stopped server was running or had waiting is gone with it — save
/// this one record, which the drop itself wrote on the device before its first
/// page landed, precisely so that the pages are packed rather than left for a
/// sync to carry in one Container each. A server takes them up once its
/// catch-up as it starts has finished, and again each time the Library is
/// unlocked in place, since a lock stops a freeze that had not begun (spec:
/// DK-2). Each is armed through [`freeze_folder`], the queue every other
/// freeze goes through, so the explorer shows a resumed book being packed
/// exactly as it shows one just dropped, and a book already running or waiting
/// is joined rather than queued twice.
///
/// Read without a key handle: what is read is the device's own record, not
/// the Library, and a server starting up is nobody being here (spec: DK-4).
/// The freeze the reservation arms takes the keys when its turn comes, as any
/// does.
///
/// A reservation whose drop never finished receiving is passed over: it names
/// the pages that landed before the process taking the drop stopped, not the
/// book, and packing those would leave the rest — which the person drops again
/// — for a second Pack. It stays as the drop in progress it is, and the next
/// drop onto its folder takes it over and packs the whole of it. Each start
/// says how many there are, so the files they hold, which the listing shows as
/// received and waiting, are not waiting silently. A drop still landing in this
/// process is one of these too: it arms its own run once it stops receiving.
///
/// A catalog that will not answer is said and leaves every reservation where
/// it is, for the next start or unlock.
pub async fn resume_reserved(state: &Arc<ServerState>) {
    let Ok(library) = state.unlocked_unattended() else {
        return;
    };
    let reservations = match library.reservations().await {
        Ok(reservations) => reservations,
        Err(refused) => {
            warn!(
                operation = "freeze",
                reason = %refused.redacted(),
                "the reservations of drops waiting for their Pack could not be read",
            );
            return;
        }
    };
    let (received, in_progress): (Vec<_>, Vec<_>) = reservations
        .into_iter()
        .partition(|reservation| reservation.received);
    // Counts only: a reservation names a folder and the files in it, which are
    // the user's own names (spec: EL-1). A drop still landing on a handle taken
    // before a lock is not one cut off, and not counted as one.
    let cut_off: Vec<_> = in_progress
        .iter()
        .filter(|reservation| !library.live.is_live(reservation.id))
        .collect();
    if !cut_off.is_empty() {
        info!(
            operation = "freeze",
            reservations = cut_off.len(),
            paths = cut_off
                .iter()
                .map(|reservation| reservation.paths.len())
                .sum::<usize>(),
            "drops cut off part way are in progress; dropping their folders again finishes them",
        );
    }
    if received.is_empty() {
        return;
    }
    info!(
        operation = "freeze",
        reservations = received.len(),
        paths = received
            .iter()
            .map(|reservation| reservation.paths.len())
            .sum::<usize>(),
        "taking up the drops waiting for their Pack",
    );
    for reservation in received {
        // One marked a moment ago by a drop on a handle taken before a lock,
        // which arms its own run as it lets go of it (spec: PK-17).
        if library.live.is_live(reservation.id) {
            continue;
        }
        freeze_folder(Arc::clone(state), Book::reserved(reservation));
    }
}
