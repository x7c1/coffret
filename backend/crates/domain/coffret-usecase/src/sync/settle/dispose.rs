use coffret_model::Redacted;
use tracing::{debug, info, warn};

use crate::commit::CommitPolicy;
use crate::device_state::PendingRow;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::spool::Spool;
use crate::sync::disposal::Disposal;
use crate::sync::settled::Settled;
use crate::sync::sync_error::SyncResult;

/// Disposes only of provenance proven not to have committed: abandoned before
/// any commit attempt, or attempted only into slots that each hold another
/// record (spec: OC-2, OC-3). The caller holds exclusive ownership, so no live
/// producer can resume it.
pub(super) async fn dispose(
    store: &dyn ObjectStore,
    index: &dyn Index,
    spool: &dyn Spool,
    policy: &CommitPolicy,
    row: PendingRow,
) -> SyncResult<Settled> {
    spool.discard(&row.spool_path).await?;

    let disposal = match row.state.object_ref() {
        Some(object) => match policy.retry.run("trash", || store.trash(object)).await {
            Ok(()) => {
                info!(
                    container = %row.container_id,
                    batch = %row.batch,
                    "trashed a Container an interrupted run uploaded and never committed",
                );
                Disposal::Trashed
            }
            // Retain the row on refusal: it remains the proof that makes
            // retrying this cleanup safe (spec: OC-2, OC-3).
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
                "an interrupted run's spool never left the device",
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
