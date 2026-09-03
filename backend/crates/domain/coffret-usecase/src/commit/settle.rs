use coffret_format::{
    decode_index_snapshot, encode_control_object, encode_index_snapshot, ControlEncodeRequest,
    IndexSnapshotPayload,
};
use coffret_model::{
    ContainerId, ControlObjectKind, ControlObjectName, Generation, JournalRecord, Redacted,
};
use tracing::{debug, info, warn};

use crate::byte_stream::ByteStream;
use crate::commit::checkpoint_outcome::CheckpointOutcome;
use crate::commit::commit_error::{CommitError, CommitResult, ControlObjectFault};
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::control_object;
use crate::commit::untrashed_removal::UntrashedRemoval;
use crate::commit_slot::CommitSlot;
use crate::error::Error;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// How many times the checkpoint upload is attempted against a slot that keeps
/// refusing while holding nothing.
///
/// A refusal settles nothing (spec: CP-3), so a slot found empty after one is
/// tried again rather than reported (spec: CK-11). The cap is what stops that
/// from being a loop: a checkpoint that never lands leaves the commit valid and
/// the next qualifying moment writes one (spec: CK-8).
const CHECKPOINT_ATTEMPTS: u32 = 3;

/// Moves what the batch removed out of the way (spec: CP-14, OC-6).
///
/// The record is already the truth about which Containers are current, so a
/// removal that cannot be trashed does not un-commit anything: it leaves an
/// object no current state names, which a later run can still reach. That is
/// why the failures come back rather than stopping the settle — trashing is
/// recoverable and so is failing to — and they come back with their reasons,
/// because a later run finishing the job acts on the refusal and not on the
/// Container ID alone.
///
/// Trash and not purge: removing a Container is meant to be recoverable by a
/// person, and irreversible deletion is what Master Key rotation does to
/// old-epoch control objects and nothing else (spec: MR-3).
pub(super) async fn trash_removals(
    store: &dyn ObjectStore,
    policy: &CommitPolicy,
    listing: &ControlListing,
    removals: &[ContainerId],
) -> Vec<UntrashedRemoval> {
    let mut untrashed = Vec::new();
    for container_id in removals {
        // The handle comes from the listing the catch-up already read, because
        // a store that mints identifiers does not name an object by its name
        // and a Container's name is all a record carries (spec: FM-3).
        let Some(object) = listing.container(*container_id) else {
            debug!(
                container = %container_id,
                "the removed Container's object is not in Storage; nothing to trash",
            );
            continue;
        };
        match policy.retry.run("trash", || store.trash(object)).await {
            Ok(()) => {}
            Err(error) => {
                warn!(
                    container = %container_id,
                    reason = %error.redacted(),
                    "the commit stands, but the removed Container is still in Storage",
                );
                untrashed.push(UntrashedRemoval {
                    container_id: *container_id,
                    cause: error,
                });
            }
        }
    }
    untrashed
}

/// Writes the Snapshot checkpointing the head this commit became, if the policy
/// asks for one (spec: CK-8, CK-10).
///
/// The judgement is the committing device's, made after its commit, over the
/// stretch of Journal it has just replayed: how much has been committed since
/// the newest checkpoint. A Snapshot is not written after every commit — a
/// commit pays for its own batch, never for the whole Library's Index.
///
/// It goes into the one slot the record reserved for it and nowhere else
/// (spec: CK-10, CP-15). Losing that conditional create is not a failure: two
/// Snapshots of one head are the same checkpoint, so a valid Snapshot of this
/// head already there settles it (spec: CK-11). Anything else at the slot is
/// reported and neither overwritten nor written under another name, because a
/// second name for one head would leave readers two checkpoints to choose
/// between.
pub(super) async fn write_checkpoint(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    record: &JournalRecord,
    slot: &CommitSlot,
    newest_checkpoint: Option<Generation>,
) -> CheckpointOutcome {
    let uncovered = match newest_checkpoint {
        Some(covered) => record.generation.get().saturating_sub(covered.get()),
        // Nothing checkpoints this Library yet, so every record since its first
        // one stands uncovered (spec: FM-13).
        None => record.generation.get().saturating_add(1),
    };
    if uncovered <= policy.checkpoint_threshold {
        debug!(
            uncovered,
            threshold = policy.checkpoint_threshold,
            "the Journal past the newest checkpoint is within the threshold",
        );
        return CheckpointOutcome::NotDue;
    }

    match checkpoint(store, index, keys, policy, record, slot).await {
        Ok(outcome) => outcome,
        Err(cause) => {
            warn!(
                generation = record.generation.get(),
                reason = %cause.redacted(),
                "the commit stands, but its checkpoint was not written",
            );
            CheckpointOutcome::Failed {
                cause: Box::new(cause),
            }
        }
    }
}

/// Encodes the Index and puts it in the record's snapshot slot.
async fn checkpoint(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    record: &JournalRecord,
    slot: &CommitSlot,
) -> CommitResult<CheckpointOutcome> {
    let name = ControlObjectName::index_snapshot(record.generation);
    let content = index.snapshot().await?;
    let payload = encode_index_snapshot(&IndexSnapshotPayload::ordinary(content))?;
    let object = encode_control_object(&ControlEncodeRequest::new(
        &name,
        ControlObjectKind::IndexSnapshot,
        keys.of_kind(ControlObjectKind::IndexSnapshot),
        &payload,
    ))?;

    for _ in 0..CHECKPOINT_ATTEMPTS {
        match policy
            .retry
            .run("put_if_absent", || {
                store.put_if_absent(slot, ByteStream::from(object.bytes().to_vec()))
            })
            .await
        {
            Ok(_) => {
                info!(
                    object = %name,
                    generation = record.generation.get(),
                    "wrote the checkpoint of the head this commit became",
                );
                return Ok(CheckpointOutcome::Written { object: name });
            }
            Err(Error::AlreadyExists { .. }) => {
                if let Some(outcome) = sibling(store, keys, policy, record, slot, &name).await? {
                    return Ok(outcome);
                }
                // The slot holds nothing, so the refusal settled nothing
                // (spec: CP-3, CK-11): try again rather than report.
                debug!(
                    object = %name,
                    "the snapshot slot refused a create while holding nothing; trying again",
                );
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(CommitError::Storage(Error::AlreadyExists {
        object: slot.name().to_owned(),
    }))
}

/// What is at the snapshot slot after a refusal, if anything (spec: CK-11).
///
/// `None` says the slot holds nothing, which is not "anything else": the
/// refusal settled nothing and the upload is worth attempting again.
async fn sibling(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    record: &JournalRecord,
    slot: &CommitSlot,
    name: &ControlObjectName,
) -> CommitResult<Option<CheckpointOutcome>> {
    let object = store.object_at(slot)?;
    let decoded = match control_object::read(store, &policy.retry, keys, name, &object).await {
        Ok(decoded) => decoded,
        Err(CommitError::Storage(Error::NotFound { .. })) => return Ok(None),
        Err(CommitError::Format(error)) => {
            return Err(CommitError::CorruptControlObject {
                object: name.clone(),
                fault: ControlObjectFault::Unopenable(error),
            })
        }
        Err(error) => return Err(error),
    };
    let payload = decode_index_snapshot(&decoded.payload, decoded.kind)?;
    let stands_at = payload.content.checkpoint.head_generation;
    if stands_at != record.generation {
        return Err(CommitError::CorruptControlObject {
            object: name.clone(),
            fault: ControlObjectFault::CheckpointsAnotherHead { found: stands_at },
        });
    }
    debug!(
        object = %name,
        generation = record.generation.get(),
        "another device had already checkpointed this head",
    );
    Ok(Some(CheckpointOutcome::Existing {
        object: name.clone(),
    }))
}
