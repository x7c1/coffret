use coffret_model::{ObjectRef, Redacted};
use tracing::{debug, info, warn};

use super::SettleStorage;
use crate::commit::CommitPolicy;
use crate::device_state::{PendingRow, SpoolState};
use crate::index::Index;
use crate::spool::Spool;
use crate::sync::disposal::Disposal;
use crate::sync::settled::Settled;
use crate::sync::sync_error::SyncResult;

/// Disposes only of provenance proven not to have committed: abandoned before
/// any commit attempt, or attempted only into slots that each hold another
/// record (spec: OC-2, OC-3). The caller holds exclusive ownership, so no live
/// producer can resume it.
///
/// The row is removed only once nothing of its Container can be on Storage:
/// its object trashed, or shown absent. A refused trash retains the row, since
/// it remains the proof that makes retrying this cleanup safe (spec: OC-2,
/// OC-3).
pub(super) async fn dispose(
    storage: &SettleStorage<'_>,
    index: &dyn Index,
    spool: &dyn Spool,
    policy: &CommitPolicy,
    row: PendingRow,
) -> SyncResult<Settled> {
    spool.discard(&row.spool_path).await?;

    let disposal = match on_storage(storage, &row) {
        Some(object) => match policy
            .retry
            .run("trash", || storage.store.trash(object))
            .await
        {
            Ok(()) => {
                info!(
                    container = %row.container_id,
                    batch = %row.batch,
                    "trashed a Container an interrupted run uploaded and never committed",
                );
                Disposal::Trashed
            }
            Err(error) => {
                warn!(
                    container = %row.container_id,
                    batch = %row.batch,
                    reason = %error.redacted(),
                    "an abandoned Container is still in Storage",
                );
                Disposal::LeftInStorage { cause: error }
            }
        },
        None => {
            debug!(
                container = %row.container_id,
                batch = %row.batch,
                "an interrupted run's Container never reached Storage",
            );
            Disposal::NeverUploaded
        }
    };

    if !matches!(disposal, Disposal::LeftInStorage { .. }) {
        index.clear_pending_row(row.container_id).await?;
    }
    Ok(Settled::Disposed {
        container_id: row.container_id,
        disposal,
    })
}

/// The object an abandoned row's Container may have left on Storage, or `None`
/// where nothing of it is there.
///
/// A recorded handle is the answer. A whole spool with no handle is not proof
/// that the Container never left the device: a run records the handle only
/// after its put returns, so one killed between the two leaves an object no row
/// names (spec: OC-2). Its name is the one the put used, drawn from the
/// Container ID (spec: FM-3), and the walk this run's catch-up took is what
/// turns that name into a handle on a store that mints identifiers, as it is
/// for a removal (spec: OC-6). Not being in that walk is the absence being
/// sought, since this device exclusively owns the row and nothing else puts its
/// Container. Only a row still `Spooling` is provably never uploaded, because a
/// run puts a Container only out of a finished spool.
fn on_storage<'a>(storage: &'a SettleStorage<'_>, row: &'a PendingRow) -> Option<&'a ObjectRef> {
    match &row.state {
        SpoolState::Spooled(Some(object)) => Some(object),
        SpoolState::Spooled(None) => {
            let found = storage.caught.listing.container(row.container_id);
            if found.is_some() {
                info!(
                    container = %row.container_id,
                    batch = %row.batch,
                    "an interrupted run put a Container it never recorded; trashing it by name",
                );
            }
            found
        }
        SpoolState::Spooling => None,
    }
}
