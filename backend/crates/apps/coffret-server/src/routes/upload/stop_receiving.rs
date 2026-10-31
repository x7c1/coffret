use std::sync::Arc;

use coffret_device::OpenLibrary;

use crate::adds::{log_undone, undo, AddId};
use crate::state::ServerState;
use crate::sync::arm_sync;

use super::drop_reservation::DropReservation;
use super::release::release_stopped;

/// What an add a person stopped while its files were still arriving comes to
/// (spec: LA-36), by whichever route it arrived.
///
/// A Pack drop is undone here and now. Its files are reserved, and the
/// reservation is still held live by this request, so no freeze packs them
/// and no sync carries them in while they are being removed (spec: PK-17);
/// the reservation is then let go as for any drop that armed nothing — one it
/// took over from a drop cut off before it is kept, for that drop's files.
///
/// A drop for a sync is not: a sync under way may walk the folders this drop
/// wrote into — every sync not armed by a drop walks every mapped folder — and
/// so be about to read these very files. So the add is handed to the sync,
/// stopped: the run carrying it undoes it under its turn at this device's
/// pending work, before it scans anything (see [`sync`](crate::sync)), and a
/// run already under way is asked to stop as well, so it does not commit what
/// is being taken away. The run it arms here walks the folders the drop wrote
/// into, as for a drop that finished (spec: EP-10). A drop that had written
/// nothing has nothing to take away, and is answered here either way.
pub(super) async fn stop_receiving(
    state: &Arc<ServerState>,
    library: &OpenLibrary,
    id: AddId,
    reservation: Option<DropReservation>,
    for_sync: bool,
    wrote_any: bool,
    operation: &'static str,
) {
    if for_sync && wrote_any {
        state.adds.armed(&id, None, Default::default());
        arm_sync(Arc::clone(state), Some(id));
        state.syncs.ask_stop();
        return;
    }
    if let Some(add) = state.adds.get(&id) {
        let undone = undo(library, add).await;
        release_stopped(library, reservation, operation).await;
        log_undone(&state.activity, undone);
        state.adds.end(&id);
    }
}
