use tokio::fs;
use tracing::info;

use crate::local_error::LocalError;
use crate::local_operation::LocalOperation;
use crate::spool_file;
use crate::spooled_container::commit_spooled;
use crate::sync::reconciled::Reconciled;
use crate::sync::sync_error::SyncResult;
use crate::sync::sync_outcome::SyncOutcome;
use crate::sync::sync_request::SyncRequest;
use crate::sync::{reconcile, scan, spool};
use crate::upload;

/// Carries every changed file under this device's mapped folders into the
/// Library.
///
/// The whole path, in the order it has to happen in: settle the pending rows an
/// interrupted run left (spec: OC-2, OC-3, OC-7), scan the mapped folders against
/// the Index (spec: EP-9, EP-10), encode what is new or changed into Containers
/// of its own (spec: FM-1, FM-2, FM-3, FM-4, FM-5, FM-6, FM-7, FM-8, FM-9,
/// PK-15), spool and upload them (spec: OC-2), and commit the batch
/// (spec: CP-1). What the Library becomes is decided by
/// [`commit_batch`](crate::commit::commit_batch), and everything before it
/// changes nothing about the Library: a run that fails short of the Journal
/// record leaves spools, and perhaps objects, that this device's own pending
/// rows account for.
///
/// Settling comes first because the scan reads what the settling decides. A row
/// of this device's own is either a batch that never committed or a commit whose
/// record landed and whose Index refresh did not, and a scan that ran with that
/// open would read a stale answer for a path this device has already committed —
/// spooling and uploading the file again, only for the commit's own catch-up to
/// refuse the batch as a collision with the Entry it made current (spec: EP-6).
///
/// Two kinds of file are reported and not acted on: one whose current Entry
/// lives in a Pack, and one this device had and no longer has. Neither is
/// skipped quietly, and neither is an error — so a caller reads
/// [`SyncOutcome::deferred`]. A run that returns successfully with findings in
/// it has *not* backed up every local file (spec: PK-14).
///
/// A run with nothing to upload commits nothing rather than committing an empty
/// batch: a Journal record is a generation, and spending one on a batch that
/// changes no Container would make every device replay a record that says
/// nothing (spec: CP-1).
pub async fn sync_folders(request: SyncRequest<'_>) -> SyncResult<SyncOutcome> {
    let SyncRequest {
        store,
        index,
        keys,
        spool_dir,
        batch,
        now,
        policy,
    } = request;

    let reconciled = reconcile::reconcile(store, index, keys.control(), &policy, now).await?;

    let survey = scan::scan(index, now).await?;
    fs::create_dir_all(&spool_dir)
        .await
        .map_err(|cause| LocalError::io(LocalOperation::Creating, &spool_dir, cause))?;

    let mut spooled = Vec::with_capacity(survey.candidates.len());
    for candidate in &survey.candidates {
        spooled.push(spool::spool(index, keys, &spool_dir, &batch, now, candidate).await?);
    }
    upload::upload(store, index, &policy.retry, &batch, now, &mut spooled).await?;

    // What this device saw of a file it did not have to upload is its own
    // bookkeeping, and belongs to it whether or not this run commits anything
    // (spec: EP-10).
    for observation in survey.refreshed {
        index.mark_present(observation).await?;
    }

    let commit = commit_spooled(store, index, keys.control(), &policy, now, &spooled).await?;
    if commit.is_some() {
        // The commit's refresh has already dropped their pending rows
        // (spec: OC-2), so the ciphertext on this device is the last thing
        // left of the batch.
        for container in &spooled {
            spool_file::discard(&container.spool_path).await?;
        }
    }

    let outcome = SyncOutcome {
        added: spooled
            .iter()
            .map(|container| container.container_id)
            .collect(),
        replaced: spooled
            .iter()
            .flat_map(|container| container.replaces.iter().copied())
            .collect(),
        unchanged: survey.unchanged,
        deferred: survey.deferred,
        reconciled,
        commit,
    };
    let completed = outcome
        .reconciled
        .iter()
        .filter(|one| matches!(one, Reconciled::Completed { .. }))
        .count();
    info!(
        added = outcome.added.len(),
        replaced = outcome.replaced.len(),
        unchanged = outcome.unchanged,
        deferred = outcome.deferred.len(),
        completed,
        disposed = outcome.reconciled.len() - completed,
        "a sync run finished",
    );
    Ok(outcome)
}
