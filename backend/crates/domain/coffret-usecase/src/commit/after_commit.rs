use coffret_format::{
    decode_index_snapshot, encode_control_object, encode_index_snapshot, ControlEncodeRequest,
    IndexSnapshotPayload,
};
use coffret_model::{
    ContainerId, ControlObjectKind, ControlObjectName, Generation, IndexCheckpoint, JournalRecord,
    ObjectRef, Redacted,
};
use tracing::{debug, info, warn};

use crate::byte_stream::ByteStream;
use crate::commit::checkpoint_failure::CheckpointFailure;
use crate::commit::checkpoint_outcome::CheckpointOutcome;
use crate::commit::commit_error::{CommitError, CommitResult, ControlObjectFault};
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::control_object;
use crate::commit::untrashed_removal::UntrashedRemoval;
use crate::control_head::ControlHead;
use crate::error::Error;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// How many times the checkpoint upload is attempted against a slot that keeps
/// refusing while holding nothing.
///
/// A refusal decides nothing (spec: CP-3), so a slot found empty after one is
/// tried again rather than reported (spec: CK-11). The cap is what stops that
/// from being a loop: a checkpoint that never lands leaves the commit valid and
/// the next qualifying moment writes one (spec: CK-8).
const CHECKPOINT_ATTEMPTS: u32 = 3;

/// Moves what the batch removed out of the way (spec: CP-14, OC-6).
///
/// The record is already the truth about which Containers are current, so a
/// removal that cannot be trashed does not un-commit anything: it leaves an
/// object no current state names, which a later run can still reach. That is
/// why the failures come back rather than stopping the step after the commit —
/// trashing is recoverable and so is failing to — and they come back with their
/// reasons, because a later run finishing the job acts on the refusal and not on
/// the Container ID alone.
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
/// It goes into the one slot the record carries for it and nowhere else
/// (spec: CK-10, CP-15): the identifier in the record's `snapshot_slot` where
/// the Storage mints identifiers, and the name `idx-<generation>` re-derived
/// from the record's generation where it does not. Consuming what the record
/// carries, rather than a reservation of this device's own, is what makes every
/// device that checkpoints this head aim at one object.
///
/// Losing that conditional create is not a failure: two Snapshots of one head
/// are the same checkpoint, so a valid Snapshot of this head already there
/// answers it (spec: CK-11). Anything else at the slot is reported and neither
/// overwritten nor written under another name, because a second name for one
/// head would leave readers two checkpoints to choose between.
pub(super) async fn write_checkpoint(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    record: &JournalRecord,
    newest_checkpoint: Option<Generation>,
) -> CheckpointOutcome {
    let uncovered = match newest_checkpoint {
        Some(covered) => record.generation().get().saturating_sub(covered.get()),
        // Nothing checkpoints this Library yet, so every record since its first
        // one stands uncovered (spec: FM-13).
        None => record.generation().get().saturating_add(1),
    };
    if uncovered <= policy.checkpoint_threshold {
        debug!(
            uncovered,
            threshold = policy.checkpoint_threshold,
            "the Journal past the newest checkpoint is within the threshold",
        );
        return CheckpointOutcome::NotDue;
    }

    match checkpoint(store, index, keys, policy, record).await {
        Ok(Checkpointed {
            object,
            written: true,
            ..
        }) => CheckpointOutcome::Written { object },
        Ok(Checkpointed { object, .. }) => CheckpointOutcome::Existing { object },
        Err(cause) => {
            warn!(
                generation = record.generation().get(),
                reason = %cause.redacted(),
                "the commit stands, but its checkpoint was not written",
            );
            CheckpointOutcome::Failed {
                cause: CheckpointFailure::of_checkpoint_write(cause),
            }
        }
    }
}

/// The ordinary Index Snapshot of one head as it stands on Storage, and the
/// checkpoint it records.
pub(crate) struct Checkpointed {
    /// The name it stands under, `idx-<generation>` (spec: CK-10).
    pub(crate) object: ControlObjectName,
    /// Whether this device wrote it, rather than finding the one another device
    /// had already put in the slot (spec: CK-11).
    pub(crate) written: bool,
    /// What it records: the head it represents, the last Journal generation it
    /// applies, and the Keyring tuple it depends on (spec: CK-1, CK-3).
    pub(crate) checkpoint: IndexCheckpoint,
}

/// Makes sure the ordinary Snapshot of the head `record` names stands on Storage,
/// and answers with it: the one already in the head's snapshot slot where the
/// listing names one, and otherwise the Index written there (spec: CK-8, CK-10,
/// CK-11).
///
/// What `prune` rests on, and why it reads before it writes: a Snapshot is the
/// whole Library's Index, multi-megabyte, and a conditional create that loses
/// has still sent it — so where the listing already says the checkpoint
/// exists, it is read rather than uploaded again to be refused. A listed
/// Snapshot gone by the time it is read is written afresh, as a slot holding
/// nothing is after a refusal (spec: CK-11). Anything else at the name is
/// reported, never overwritten, for the reason the commit's checkpoint gives.
///
/// The Index has to stand at `record`'s head, which the catch-up a caller ran
/// first is what ensures: the Snapshot written is that Index (spec: CK-7).
pub(crate) async fn checkpoint_head(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    listing: &ControlListing,
    record: &JournalRecord,
) -> CommitResult<Checkpointed> {
    let name = ControlObjectName::index_snapshot(record.generation());
    if let Some(object) = listing.handle(&name.to_string()) {
        if let Some(checkpoint) = existing(store, keys, policy, record, &name, object).await? {
            debug!(
                object = %name,
                generation = record.generation().get(),
                "this head is already checkpointed",
            );
            return Ok(Checkpointed {
                object: name,
                written: false,
                checkpoint,
            });
        }
    }
    checkpoint(store, index, keys, policy, record).await
}

/// Encodes the Index and puts it in the record's snapshot slot.
async fn checkpoint(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    record: &JournalRecord,
) -> CommitResult<Checkpointed> {
    let slot = ControlHead::at(record.generation()).snapshot_slot(record.snapshot_slot());
    let name = ControlObjectName::index_snapshot(record.generation());
    let content = index.snapshot().await?;
    let recorded = content.checkpoint().clone();
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
                store.put_if_absent(&slot, ByteStream::from(object.bytes().to_vec()))
            })
            .await
        {
            Ok(_) => {
                info!(
                    object = %name,
                    generation = record.generation().get(),
                    "wrote the checkpoint of this head",
                );
                return Ok(Checkpointed {
                    object: name,
                    written: true,
                    checkpoint: recorded,
                });
            }
            Err(Error::AlreadyExists { .. }) => {
                let object = store.object_at(&slot)?;
                if let Some(checkpoint) =
                    existing(store, keys, policy, record, &name, &object).await?
                {
                    debug!(
                        object = %name,
                        generation = record.generation().get(),
                        "another device had already checkpointed this head",
                    );
                    return Ok(Checkpointed {
                        object: name,
                        written: false,
                        checkpoint,
                    });
                }
                // The slot holds nothing, so the refusal decided nothing
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

/// The checkpoint the Snapshot standing at `object` records, if anything stands
/// there (spec: CK-11).
///
/// Read after a refusal of the snapshot slot, and by `prune` before it writes
/// one. `None` says the slot holds nothing, which is not "anything else": the
/// refusal decided nothing and the upload is worth attempting again.
async fn existing(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    record: &JournalRecord,
    name: &ControlObjectName,
    object: &ObjectRef,
) -> CommitResult<Option<IndexCheckpoint>> {
    let decoded = match control_object::read(store, &policy.retry, keys, name, object).await {
        Ok(decoded) => decoded,
        Err(CommitError::Storage(Error::NotFound { .. })) => return Ok(None),
        // A length this build cannot address is this build's limit and never a
        // corrupt object, so it goes out as it came rather than as an accusation
        // about what stands at the slot. No control object states one today.
        Err(CommitError::Format(error))
            if !matches!(
                error,
                coffret_format::Error::UnaddressableOnThisBuild { .. }
            ) =>
        {
            return Err(CommitError::CorruptControlObject {
                object: name.clone(),
                fault: ControlObjectFault::Unopenable(error),
            })
        }
        Err(error) => return Err(error),
    };
    // The name's own generation goes to the decoder, which is where the rule
    // that a Snapshot checkpoints the head it is named for is held (spec:
    // CK-10). A sibling that checkpoints another head is refused there, and
    // arrives here as the object being unopenable at this position — the same
    // answer this flow gave when it made the comparison itself.
    let payload = decode_index_snapshot(&decoded.payload, decoded.kind, record.generation())
        .map_err(|error| CommitError::CorruptControlObject {
            object: name.clone(),
            fault: ControlObjectFault::Unopenable(error),
        })?;
    Ok(Some(payload.content.checkpoint().clone()))
}
