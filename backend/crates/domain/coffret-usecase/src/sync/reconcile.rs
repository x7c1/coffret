use std::collections::BTreeSet;

use coffret_model::ContainerId;
use tracing::{debug, info, warn};

use crate::commit::CommitPolicy;
use crate::device_state::PendingUpload;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::reconciled::Reconciled;
use crate::sync::spool;
use crate::sync::sync_error::SyncResult;

/// Disposes of what an interrupted run left behind.
///
/// # Why a spool is never resumed into a batch
///
/// A Container is opened by a Container Key drawn for it alone (spec: KD-2),
/// and the one place that key is ever written down is the Key Envelope the
/// commit puts in the Keyring (spec: FM-14, KL-7) — which is exactly the step
/// an interrupted run did not reach. So a spool an earlier run left behind is
/// ciphertext with no key anywhere on this device or on Storage: adopting it
/// into a new batch would commit a Container nothing can ever open, and a
/// second place to keep key material is not a trade worth making to avoid
/// re-encrypting a file.
///
/// What a run does instead is dispose of it and let its own scan spool the
/// source file again. That converges for the reason it needs to: the Entry ends
/// up committed exactly once, under a Container the committed Keyring maps, and
/// neither the spool file nor the row survives to be found a third time.
///
/// # Why it runs last, and what `caught_up` decides
///
/// Trashing an uploaded object rests on the Container not being current, and
/// what makes that answerable is a caught-up Index. A commit catches the Index
/// up before it does anything else (spec: CK-9), so a run that committed knows
/// the current set at this point — and that, and nothing else, is what
/// `caught_up` says. The case it exists for is an earlier run whose record
/// landed and whose own refresh did not: its row survives for a Container that
/// *is* current, and this device's Index does not know it yet. Reading that
/// Index would say no record names the Container and trash one the Library
/// holds.
///
/// A run that uploaded nothing commits nothing and so reads no head, which is
/// exactly when that mistake is possible. Such a run therefore leaves an
/// uploaded Container alone — object, spool, and row together, because the row
/// is the local provenance that makes the disposal possible at all (spec: OC-2,
/// OC-3) — for a later run whose commit reads the head. A row that names no
/// object is not waiting on any of that: nothing that was never uploaded can be
/// current, so its spool goes and its row goes whatever the Index knows.
pub(super) async fn reconcile(
    store: &dyn ObjectStore,
    index: &dyn Index,
    policy: &CommitPolicy,
    caught_up: bool,
) -> SyncResult<Vec<Reconciled>> {
    // What this run committed is not among these: the commit's own refresh
    // dropped the rows of the Containers its record names (spec: OC-2), so what
    // is left is the batches that never got one.
    let pending = index.pending_uploads().await?;
    if pending.is_empty() {
        return Ok(Vec::new());
    }

    let current: BTreeSet<ContainerId> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| container.id)
        .collect();

    let mut reconciled = Vec::with_capacity(pending.len());
    for row in pending {
        if row.object_ref.is_some() && !caught_up {
            debug!(
                container = %row.container_id,
                batch = %row.batch,
                "left an uploaded Container to a run whose commit reads the Library's head",
            );
            continue;
        }
        reconciled.push(dispose(store, index, policy, &current, row).await?);
    }
    Ok(reconciled)
}

/// Deletes one abandoned spool, and its object where nothing names it.
async fn dispose(
    store: &dyn ObjectStore,
    index: &dyn Index,
    policy: &CommitPolicy,
    current: &BTreeSet<ContainerId>,
    row: PendingUpload,
) -> SyncResult<Reconciled> {
    spool::discard(&row.spool_path).await?;

    let trashed = match &row.object_ref {
        Some(object) if !current.contains(&row.container_id) => {
            match policy.retry.run("trash", || store.trash(object)).await {
                Ok(()) => {
                    info!(
                        container = %row.container_id,
                        batch = %row.batch,
                        "trashed a Container an interrupted run uploaded and never committed",
                    );
                    true
                }
                // The row is about to go, so the provenance this rested on goes
                // with it; what is left is an object no current state names,
                // which is orphan cleanup's to find and a person's to decide on
                // (spec: OC-1, OC-4).
                Err(error) => {
                    warn!(
                        container = %row.container_id,
                        reason = %error,
                        "an abandoned Container is still in Storage",
                    );
                    false
                }
            }
        }
        Some(_) => {
            debug!(
                container = %row.container_id,
                "an earlier run's commit landed after all; its object is the Library's",
            );
            false
        }
        None => {
            debug!(
                container = %row.container_id,
                batch = %row.batch,
                "an interrupted run's spool never left the device",
            );
            false
        }
    };

    index.clear_pending_upload(row.container_id).await?;
    Ok(Reconciled {
        container_id: row.container_id,
        trashed,
    })
}
