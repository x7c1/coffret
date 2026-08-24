use coffret_model::{ContainerAddition, ContainerKind, ContainerSummary};
use tokio::fs;
use tracing::info;

use crate::commit::{
    commit_batch, CommitOutcome, CommitPolicy, CommitRequest, ControlKeys, PreparedAddition,
    PreparedBatch,
};
use crate::device_state::{DeviceTime, LocalObservation};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::sync::spooled::Spooled;
use crate::sync::sync_error::{LocalOperation, SyncError, SyncResult};
use crate::sync::sync_outcome::SyncOutcome;
use crate::sync::sync_request::SyncRequest;
use crate::sync::{reconcile, scan, spool, upload};

/// Carries every changed file under this device's mapped folders into the
/// Library.
///
/// The whole path, in the order it has to happen in: scan the mapped folders
/// against the Index (spec: EP-9, EP-10), encode what is new or changed into
/// Containers of its own (spec: FM-1 to FM-9, PK-15), spool and upload them
/// (spec: OC-2), and commit the batch (spec: CP-1). What the Library becomes is
/// decided by [`commit_batch`], and everything before it changes nothing about
/// the Library: a run that fails short of the Journal record leaves spools, and
/// perhaps objects, that this device's own pending rows account for.
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

    let survey = scan::scan(index, now).await?;
    fs::create_dir_all(&spool_dir)
        .await
        .map_err(|cause| SyncError::Io {
            operation: LocalOperation::Creating,
            path: spool_dir.clone(),
            cause,
        })?;

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

    let commit = commit_uploads(store, index, keys.control(), &policy, now, &spooled).await?;
    if commit.is_some() {
        // The commit's refresh has already dropped their pending rows
        // (spec: OC-2), so the ciphertext on this device is the last thing
        // left of the batch.
        for container in &spooled {
            spool::discard(&container.spool_path).await?;
        }
    }
    // A commit is also the only thing that read the Library's head, which is
    // what deciding that no record names an uploaded Container takes
    // (spec: CK-9, OC-3).
    let reconciled = reconcile::reconcile(store, index, &policy, commit.is_some()).await?;

    let outcome = SyncOutcome {
        added: spooled
            .iter()
            .map(|container| container.container_id)
            .collect(),
        replaced: spooled
            .iter()
            .filter_map(|container| container.replaces)
            .collect(),
        unchanged: survey.unchanged,
        deferred: survey.deferred,
        reconciled,
        commit,
    };
    info!(
        added = outcome.added.len(),
        replaced = outcome.replaced.len(),
        unchanged = outcome.unchanged,
        deferred = outcome.deferred.len(),
        reconciled = outcome.reconciled.len(),
        "a sync run finished",
    );
    Ok(outcome)
}

/// Commits what the run uploaded, or nothing where it uploaded nothing.
async fn commit_uploads(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    now: DeviceTime,
    spooled: &[Spooled],
) -> SyncResult<Option<CommitOutcome>> {
    if spooled.is_empty() {
        return Ok(None);
    }
    let batch = PreparedBatch::adding(spooled.iter().map(addition).collect())
        .removing(spooled.iter().filter_map(|one| one.replaces).collect())
        .materializing(spooled.iter().map(|one| materialized(one, now)).collect());

    let request = CommitRequest::new(store, index, keys, batch).with_policy(policy.clone());
    Ok(Some(commit_batch(request).await?))
}

/// What the Journal record says about one Container, paired with the key that
/// opens it (spec: CP-11, KL-7).
fn addition(container: &Spooled) -> PreparedAddition {
    PreparedAddition::new(
        ContainerAddition {
            container: ContainerSummary {
                id: container.container_id,
                kind: ContainerKind::OneFile,
                ciphertext_hash: container.ciphertext_hash,
                ciphertext_len: container.ciphertext_len,
                // A cache and never evidence of membership (spec: FM-15): this
                // device holds the handle Storage answered its upload with, so
                // a reader can fetch the Container without listing first.
                object_ref: container.object_ref.clone(),
            },
            entries: vec![container.entry.clone()],
        },
        container.envelope,
    )
}

/// The local file this device put in place while producing the batch
/// (spec: EP-10).
fn materialized(container: &Spooled, at: DeviceTime) -> LocalObservation {
    LocalObservation {
        path: container.entry.path.clone(),
        size: container.entry.size,
        mtime: container.entry.mtime,
        at,
    }
}
