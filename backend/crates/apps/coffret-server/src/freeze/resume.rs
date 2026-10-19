use std::sync::Arc;

use coffret_device::Redacted;
use tracing::{info, warn};

use crate::state::ServerState;

use super::{freeze_folder, Book};

/// Takes up every drop this device recorded as waiting for its Pack, in the
/// order the drops were made (spec: PK-17, LA-12).
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
/// A reservation a drop in this process still holds live is passed over: the
/// drop arms its own run once it stops receiving. After a restart nothing is
/// live, so every reservation is taken up.
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
    if reservations.is_empty() {
        return;
    }
    // Counts only: a reservation names a folder and the files in it, which are
    // the user's own names (spec: EL-1).
    info!(
        operation = "freeze",
        reservations = reservations.len(),
        paths = reservations
            .iter()
            .map(|reservation| reservation.paths.len())
            .sum::<usize>(),
        "taking up the drops waiting for their Pack",
    );
    for reservation in reservations {
        // A drop still landing on a handle taken before a lock is not one to
        // take up: it arms its own run once it stops receiving, with every
        // page it landed (spec: PK-17).
        if library.live.is_live(reservation.id) {
            continue;
        }
        freeze_folder(Arc::clone(state), Book::reserved(reservation));
    }
}
