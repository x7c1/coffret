use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKeyStatus, KeyringMapping};
use tokio::fs;
use tracing::info;

use crate::commit::{catch_up, read_committed, ControlKeys};
use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::freeze_outcome::FreezeOutcome;
use crate::freeze::freeze_request::FreezeRequest;
use crate::freeze::frozen_pack::FrozenPack;
use crate::freeze::segment::Segment;
use crate::freeze::{scan, segment, spool};
use crate::index::Index;
use crate::local_error::LocalError;
use crate::local_operation::LocalOperation;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;
use crate::spool_file;
use crate::spooled_container::{commit_spooled, SpooledContainer};
use crate::upload;

/// Packs the eligible local files under one folder into Packs (spec: PK-17).
///
/// The whole path, in the order it has to happen in: catch the Index up to the
/// Library's head (spec: CK-9), read the committed Keyring so the run knows
/// which Containers cannot be opened at all (spec: KL-1, KL-7), scan the folder
/// for what this invocation may absorb (spec: PK-1, PK-13, PK-14), sort the
/// selection by Entry Path and cut it into Packs around the target (spec: PK-3,
/// PK-4, PK-6), spool each Pack through the streaming encoder, upload them, and
/// commit one Journal batch (spec: PK-7, CP-1).
///
/// The catch-up comes first because eligibility is a question about the current
/// Library: which Container holds each Entry, and of what kind. A run that
/// answered it from a stale catalog could absorb a one-file Container another
/// device has already replaced, or pass over a file whose Entry is no longer in
/// a Pack.
///
/// What a run may absorb is exactly PK-1's two shapes, and the shape is decided
/// by the Container kind and never by the Entry count (spec: PK-15). A file not
/// yet in the Library becomes a member of a Pack directly — a freeze does not
/// upload a one-file Container first and absorb it afterwards (spec: PK-7). A
/// file whose current Entry is held by a one-file Container is absorbed, and the
/// Pack carries the bytes on disk now, whether the file has been modified or the
/// Container's key has been lost (spec: PK-13). Anything an existing Pack holds
/// is left byte-for-byte as it is (spec: PK-2).
///
/// Two kinds of file are reported and not acted on, and neither is an error: one
/// whose Pack-held Entry the local file no longer matches, and one whose Pack
/// the Library records no key for. Both need `update`'s read-modify-replace over
/// a Pack, which this flow does not do (spec: PK-10, PK-11). Reporting them is
/// not optional — a caller reads [`FreezeOutcome::surfaced`], because a run that
/// returns successfully with findings in it has *not* packed every local file
/// current (spec: PK-14).
///
/// A run with nothing to pack commits nothing rather than committing an empty
/// batch: a Journal record is a generation, and spending one on a batch that
/// changes no Container would make every device replay a record that says
/// nothing (spec: CP-1). That is also the ordinary second run over a folder —
/// `freeze` persists no folder state, so it simply finds every file already
/// packed (spec: PK-2).
///
/// What an interrupted run leaves is the sync flow's to settle, and settled the
/// same way whatever wrote it: a pending row naming a Pack this device was
/// writing, wrote, or uploaded, which the next
/// [`sync_folders`](crate::sync::sync_folders) either disposes of or completes
/// the bookkeeping of (spec: OC-2, OC-3, OC-7). There is always a row, whatever
/// stopped the run and wherever it stopped, because the row is written before the
/// spool file it names. A spool is never resumed, here
/// or there, because the Container Key that opens it lived only in the run that
/// drew it (spec: KD-2, FM-14) — so the files a dead run packed are simply
/// eligible again.
pub async fn freeze_folder(request: FreezeRequest<'_>) -> FreezeResult<FreezeOutcome> {
    let FreezeRequest {
        store,
        index,
        keys,
        spool_dir,
        prefix,
        target,
        batch,
        now,
        policy,
    } = request;

    let key_lost = unreadable(store, index, keys.control(), &policy.retry).await?;
    let survey = scan::scan(index, prefix.as_ref(), &key_lost, now).await?;
    let segments = segment::segment(survey.selected, target)?;

    let mut spooled = Vec::with_capacity(segments.len());
    if !segments.is_empty() {
        fs::create_dir_all(&spool_dir)
            .await
            .map_err(|cause| LocalError::io(LocalOperation::Creating, &spool_dir, cause))?;
        for segment in &segments {
            spooled.push(spool::spool(index, keys, &spool_dir, &batch, now, segment).await?);
        }
        upload::upload(store, index, &policy.retry, &batch, now, &mut spooled).await?;
    }

    // What this device saw of a file it did not have to pack is its own
    // bookkeeping, and belongs to it whether or not this run commits anything
    // (spec: EP-10).
    for observation in survey.refreshed {
        index.mark_present(observation).await?;
    }

    let commit = commit_spooled(store, index, keys.control(), &policy, now, &spooled).await?;
    if commit.is_some() {
        // The commit's refresh has already dropped their pending rows
        // (spec: OC-2), so the ciphertext on this device is the last thing left
        // of the batch.
        for container in &spooled {
            spool_file::discard(&container.spool_path).await?;
        }
    }

    let outcome = FreezeOutcome {
        packs: segments
            .iter()
            .zip(&spooled)
            .map(|(segment, container)| built(segment, container, target))
            .collect(),
        absorbed: spooled
            .iter()
            .flat_map(|container| container.replaces.iter().copied())
            .collect(),
        packed_already: survey.packed_already,
        surfaced: survey.surfaced,
        commit,
    };
    info!(
        packs = outcome.packs.len(),
        entries = outcome.frozen_entries(),
        oversized = outcome.packs.iter().filter(|pack| pack.oversized).count(),
        absorbed = outcome.absorbed.len(),
        packed_already = outcome.packed_already,
        surfaced = outcome.surfaced.len(),
        "a freeze run finished",
    );
    Ok(outcome)
}

/// The Containers the committed Keyring records no key for (spec: KL-7).
///
/// A freeze has to know, for two opposite reasons. A one-file Container whose
/// key is lost is eligible whatever its content compares to — the stored
/// ciphertext is unreadable, so re-encrypting the local plaintext is the only
/// content-recovery path there is (spec: PK-11, PK-13). A Pack whose key is lost
/// is the same loss and not this flow's to repair, so it is surfaced rather than
/// passed over (spec: PK-14).
///
/// The catch-up that precedes it is what the whole run's eligibility rests on,
/// and it happens here because reading the Keyring needs the walk of Storage it
/// leaves behind (spec: CK-9, FM-12). A Library that has committed nothing has
/// no Keyring and no current Entry, so there is nothing to be lost and every
/// local file is simply new.
async fn unreadable(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    retry: &RetryPolicy,
) -> FreezeResult<BTreeSet<ContainerId>> {
    let caught = catch_up(store, index, keys, retry).await?;
    let Some(checkpoint) = index.checkpoint().await? else {
        return Ok(BTreeSet::new());
    };
    let keyring = read_committed(store, keys, retry, &caught.listing, &checkpoint.keyring).await?;
    Ok(lost(&keyring))
}

/// Which Containers of a mapping carry a key-lost marker (spec: KL-7).
fn lost(keyring: &KeyringMapping) -> BTreeSet<ContainerId> {
    keyring
        .entries
        .iter()
        .filter(|entry| entry.key == ContainerKeyStatus::KeyLost)
        .map(|entry| entry.container_id)
        .collect()
}

/// What one Pack came out as.
fn built(segment: &Segment, container: &SpooledContainer, target: u64) -> FrozenPack {
    FrozenPack {
        container_id: container.container_id,
        entries: container.entries.len(),
        footprint: segment.footprint.bytes(),
        oversized: segment.oversized(target),
    }
}
