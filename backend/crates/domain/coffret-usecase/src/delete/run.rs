use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKeyStatus, EntryPath, KeyEnvelope, KeyTable, Redacted};
use tracing::{info, warn};

use crate::commit::{catch_up, read_committed, DegradedReport};
use crate::delete::delete_error::{DeleteError, DeleteResult};
use crate::delete::delete_outcome::DeleteOutcome;
use crate::delete::delete_request::DeleteRequest;
use crate::delete::pack_refusal::PackRefusal;
use crate::delete::plan::{self, Partial};
use crate::delete::rebuilt_pack::RebuiltPack;
use crate::progress::{Phase, Step};
use crate::read_modify_replace::{rebuild, Rebuilding, Replacing};
use crate::spooled_container::{commit_spooled, SpooledContainer};
use crate::upload;

/// Deletes the Entries a selection names from the Library, in one Journal batch
/// (spec: PK-9, CP-1).
///
/// The whole path, in the order it has to happen in: catch the Index up to the
/// Library's head (spec: CK-9), read the committed Keyring for the envelopes
/// that open the Packs to rebuild and the markers that say which cannot be
/// opened (spec: KL-1, KL-7), work out from the catalog what happens to every
/// Container holding a named Entry (spec: PK-9), rebuild the Packs that keep
/// other Entries by read-modify-replace (spec: PK-10), upload the replacements,
/// and commit one batch.
///
/// What happens to a Container is decided by what else it holds. One whose
/// Entries are all named — a one-file Container holding a named Entry, or a
/// Pack holding nothing else — goes to removals, readable or not: a key-lost
/// Container leaves the current set by exactly this kind of genuine removal
/// (spec: KL-17). One that also keeps Entries is replaced by a Pack carrying
/// only the kept ones, under a new Container ID: the old one in removals, the
/// new one in additions, in the same batch (spec: PK-10, PK-12, CP-14).
///
/// Two things are refused rather than done, and both cost the one Container and
/// nothing else: a Container that keeps Entries and whose key is lost, because
/// nothing can be read back to carry them forward, and one that was read and
/// did not verify, because a replacement written from it would claim to carry
/// Entries it cannot vouch for. Either way nothing is committed for that
/// Container — every Entry it held, the named ones too, stays — and the
/// refusal is in [`DeleteOutcome::refused`] with the Entries it kept and spared
/// (spec: PK-10, KL-17). The rest of the deletion commits.
///
/// After the commit the removed objects — the outright removals and the
/// replaced Packs alike — go to the provider's trash, and one that would not go
/// leaves the commit standing and is reported in
/// [`CommitOutcome::untrashed`](crate::commit::CommitOutcome::untrashed)
/// (spec: CP-1, OC-6).
///
/// A run with nothing to delete commits nothing (spec: CP-1). What an
/// interrupted run leaves is the sync's to settle, as it settles an
/// interrupted freeze: a pending row names every rebuilt Pack from before its
/// first byte (spec: OC-2, OC-3), and says it was rebuilt rather than built out
/// of local files, so completing it after a commit that landed marks nothing
/// present on this device (spec: OC-7, EP-10).
pub async fn delete_entries(request: DeleteRequest<'_>) -> DeleteResult<DeleteOutcome> {
    let DeleteRequest {
        store,
        index,
        keys,
        spool,
        spool_dir,
        selection,
        batch,
        now,
        progress,
        policy,
    } = request;

    let _pending_owner = index.own_pending_rows().await?;

    progress.step(Step::begun(Phase::CatchingUp));
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    let (key_table, degraded) = match index.checkpoint().await? {
        Some(checkpoint) => {
            let read = read_committed(
                store,
                keys.control(),
                &policy.retry,
                &caught.listing,
                checkpoint.keyring(),
            )
            .await?;
            (Some(read.key_table), read.degraded)
        }
        // A Library that has committed nothing holds no Entry to delete
        // (spec: FM-13).
        None => (None, None),
    };
    // Said as the run ends, from wherever it ends, unless the commit's
    // examination of the same set speaks for it (spec: KL-15).
    let degraded = DegradedReport::armed(degraded);
    let key_lost = key_table.as_ref().map(lost).unwrap_or_default();

    let plan = plan::plan(index, &selection, &key_lost).await?;
    let mut refused = plan.refused;

    // Rebuilding is encoding on this device, one Pack at a time, and a Pack is
    // the unit worth counting (see `Phase::Packing`).
    let to_rebuild = plan.rebuilds.len();
    progress.step(Step::new(Phase::Packing, 0, to_rebuild));
    let mut spooled: Vec<SpooledContainer> = Vec::with_capacity(to_rebuild);
    // The Partial each spooled replacement was rebuilt from, index for index.
    let mut rebuilt: Vec<&Partial> = Vec::with_capacity(to_rebuild);
    if !plan.rebuilds.is_empty() {
        let key_table = key_table
            .as_ref()
            .expect("a Library with Entries to rebuild has committed a Keyring");
        spool.prepare_dir(&spool_dir).await?;
        let rebuilding = Rebuilding {
            store,
            index,
            keys,
            spool,
            spool_dir: &spool_dir,
            retry: &policy.retry,
            listing: &caught.listing,
            batch: &batch,
            now,
        };
        for (done, partial) in plan.rebuilds.iter().enumerate() {
            match envelope(key_table, partial.summary.id)? {
                Some(envelope) => {
                    let replacing = Replacing {
                        old: &partial.summary,
                        envelope: &envelope,
                        table: &partial.table,
                        keep: &partial.keep,
                    };
                    match rebuild(&rebuilding, &replacing).await? {
                        Ok(container) => {
                            rebuilt.push(partial);
                            spooled.push(container);
                        }
                        Err(unverified) => {
                            refused.push(partial.refusal(PackRefusal::Unverified(unverified)))
                        }
                    }
                }
                None => refused.push(partial.refusal(PackRefusal::KeyLost)),
            }
            progress.step(Step::new(Phase::Packing, done + 1, to_rebuild));
        }
        if !spooled.is_empty() {
            upload::upload(
                store,
                index,
                spool,
                &policy.retry,
                &batch,
                progress,
                &mut spooled,
            )
            .await?;
        }
    }

    let removed: Vec<ContainerId> = plan.removals.iter().map(|whole| whole.summary.id).collect();
    let commit = commit_spooled(
        store,
        index,
        keys.control(),
        &policy,
        now,
        &spooled,
        &removed,
        Some(&degraded),
        progress,
    )
    .await?;
    if commit.is_some() {
        // The commit's refresh has already dropped their pending rows
        // (spec: OC-2), so the ciphertext on this device is the last thing left
        // of the batch.
        for container in &spooled {
            if let Err(error) = spool.discard(&container.spool_path).await {
                warn!(
                    container = %container.container_id,
                    reason = %error.redacted(),
                    "a committed Pack's spool file could not be removed",
                );
            }
        }
    }

    let mut deleted: Vec<EntryPath> = Vec::new();
    let mut bytes = 0;
    let gone = plan
        .removals
        .iter()
        .flat_map(|whole| whole.entries.iter())
        .chain(rebuilt.iter().flat_map(|partial| partial.deleted()));
    for row in gone {
        deleted.push(row.path.clone());
        bytes += row.extent.size();
    }
    deleted.sort_unstable();

    let outcome = DeleteOutcome {
        deleted,
        bytes,
        removed,
        rebuilt: rebuilt
            .iter()
            .zip(&spooled)
            .map(|(partial, container)| RebuiltPack {
                replaced: partial.summary.id,
                container_id: container.container_id,
                kept: container.entries.len(),
                omitted: partial.table.len() - container.entries.len(),
                read: partial.summary.ciphertext_len.get(),
                written: container.ciphertext_len.get(),
            })
            .collect(),
        refused,
        missing: plan.missing,
        commit,
        degraded: degraded.unspoken(),
    };
    info!(
        deleted = outcome.deleted.len(),
        removed = outcome.removed.len(),
        rebuilt = outcome.rebuilt.len(),
        refused = outcome.refused.len(),
        // A count and nothing else: an Entry Path may not reach a diagnostic
        // event.
        missing = outcome.missing.len(),
        untrashed = outcome
            .commit
            .as_ref()
            .map_or(0, |commit| commit.untrashed.len()),
        keyring_degraded = outcome.degraded.is_some(),
        "a deletion finished",
    );
    Ok(outcome)
}

/// Which Containers of a key table carry a key-lost marker (spec: KL-7).
fn lost(key_table: &KeyTable) -> BTreeSet<ContainerId> {
    key_table
        .elements()
        .iter()
        .filter(|element| element.key == ContainerKeyStatus::KeyLost)
        .map(|element| element.container_id)
        .collect()
}

/// The envelope the committed Keyring maps a Container to rebuild to, or `None`
/// where it maps it to a key-lost marker.
///
/// The plan has already set aside every Container carrying a key-lost marker,
/// read off this same key table, so `None` is not expected here; should it
/// come, the caller refuses the Container as key-lost. Where the key table says
/// nothing about the Container at all, the Keyring and the catalog disagree
/// about what is current (spec: KL-7).
fn envelope(key_table: &KeyTable, container_id: ContainerId) -> DeleteResult<Option<KeyEnvelope>> {
    match key_table
        .elements()
        .iter()
        .find(|element| element.container_id == container_id)
        .map(|element| element.key)
    {
        Some(ContainerKeyStatus::Envelope(envelope)) => Ok(Some(envelope)),
        Some(ContainerKeyStatus::KeyLost) => Ok(None),
        None => Err(DeleteError::UnmappedContainer { container_id }),
    }
}
