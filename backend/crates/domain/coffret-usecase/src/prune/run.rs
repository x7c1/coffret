use coffret_model::{ControlObjectKind, ControlObjectName, Generation, Redacted};
use tracing::{debug, info, warn};

use crate::commit::{
    catch_up, checkpoint_head, fetch_header, read_records, shortfall, Checkpointed, CommitError,
    CommitPolicy, ControlListing,
};
use crate::error::Error;
use crate::object_store::ObjectStore;
use crate::prune::prune_error::PruneResult;
use crate::prune::prune_outcome::PruneOutcome;
use crate::prune::prune_refusal::PruneRefusal;
use crate::prune::prune_request::PruneRequest;
use crate::prune::unremoved_record::UnremovedRecord;

/// Removes the Journal records the Snapshot of the current head covers, all
/// but the head itself (spec: CK-4, CK-5, CK-6, CK-8).
///
/// The steps, and the rule each answers to, are listed in the
/// [module documentation](crate::prune).
///
/// A refusal is [`PruneError::Refused`](super::PruneError::Refused), and comes
/// before anything is removed; the Snapshot the run made sure of stands either
/// way. Once removing has begun nothing stops it: a record that will not go is
/// reported in [`PruneOutcome::unremoved`] and the run goes on to the next.
pub async fn prune_journal(request: PruneRequest<'_>) -> PruneResult<PruneOutcome> {
    let PruneRequest {
        store,
        index,
        keys,
        policy,
    } = request;
    let keys = keys.control();

    let caught = catch_up(store, index, keys, &policy.retry).await?;
    let Some(head) = caught.head_generation() else {
        return Err(PruneRefusal::NothingCommitted.into());
    };
    let listing = caught.listing;

    // The head's own record, for the two things only it holds: the slot its
    // Snapshot goes in, and the Keyring tuple it committed (spec: CK-10,
    // CP-10). An activation standing at the head is reported as the replay
    // reports one, rather than handled (spec: CP-5).
    let record = read_records(store, keys, &policy.retry, &listing, [head])
        .await?
        .pop()
        .ok_or(CommitError::MissingHead { generation: head })?;

    let Checkpointed {
        object: snapshot,
        written,
        checkpoint,
    } = checkpoint_head(store, index, keys, &policy, &listing, &record).await?;

    let committed = record.checkpoint();
    if checkpoint.master_key_epoch() != committed.master_key_epoch()
        || checkpoint.keyring() != committed.keyring()
    {
        warn!(
            object = %snapshot,
            recorded = checkpoint.keyring().generation().get(),
            committed = committed.keyring().generation().get(),
            "the checkpoint does not record the committed Keyring tuple; pruning nothing",
        );
        return Err(PruneRefusal::KeyringNotCommitted {
            snapshot,
            recorded_epoch: checkpoint.master_key_epoch(),
            recorded: checkpoint.keyring().clone(),
            committed_epoch: committed.master_key_epoch(),
            committed: committed.keyring().clone(),
        }
        .into());
    }
    if let Some(short) =
        shortfall(store, keys, &policy.retry, &listing, committed.keyring()).await?
    {
        short.report();
        return Err(PruneRefusal::KeyringIncomplete { snapshot, short }.into());
    }

    let covered_through = checkpoint.journal_generation();
    let mut outcome = PruneOutcome {
        head,
        snapshot,
        wrote_snapshot: written,
        covered_through,
        removed: Vec::new(),
        already_gone: Vec::new(),
        kept: Vec::new(),
        unremoved: Vec::new(),
    };
    // At or before what the Snapshot applies, and never the head itself, which
    // CK-4 makes eligible and this implementation keeps (see the module
    // documentation). Only names in the head chain are looked at, so no
    // Snapshot, Keyring replica or Container is ever a candidate (spec: CK-6).
    let eligible: Vec<Generation> = listing
        .heads()
        .filter(|generation| *generation <= covered_through && *generation < head)
        .collect();
    for generation in eligible {
        match remove(store, &policy, &listing, generation).await {
            Ok(Removal::Removed) => outcome.removed.push(generation),
            Ok(Removal::AlreadyGone) => outcome.already_gone.push(generation),
            Ok(Removal::NotARecord) => outcome.kept.push(generation),
            Err(cause) => {
                warn!(
                    generation = generation.get(),
                    reason = %cause.redacted(),
                    "an eligible Journal record was not removed; the next prune takes it",
                );
                outcome
                    .unremoved
                    .push(UnremovedRecord { generation, cause });
            }
        }
    }

    finished(&outcome);
    Ok(outcome)
}

/// What became of one eligible head position.
enum Removal {
    /// The Journal record there was moved to the trash.
    Removed,
    /// Nothing was there by the time the run came to it.
    AlreadyGone,
    /// Something other than a Journal record stands there, and was kept.
    NotARecord,
}

/// Trashes the Journal record at `generation`, if a Journal record is what
/// stands there (spec: CK-6).
///
/// The header is read first, because the name alone does not say whether the
/// object is a record or an activation Snapshot in a head position
/// (spec: FM-12): 44 bytes, read without a key, which is all the kind byte
/// needs. Trash and not purge: removing a record is meant to be recoverable by
/// a person, as removing a Container is, and irreversible deletion is what
/// Master Key rotation does to old-epoch control objects and nothing else
/// (spec: MR-3).
///
/// An object gone before its header or its trash is not a failure: another
/// device pruned it first, and the run was taking it there anyway.
async fn remove(
    store: &dyn ObjectStore,
    policy: &CommitPolicy,
    listing: &ControlListing,
    generation: Generation,
) -> Result<Removal, CommitError> {
    let name = ControlObjectName::head(generation);
    let Some(object) = listing.handle(&name.to_string()) else {
        return Ok(Removal::AlreadyGone);
    };
    let header = match fetch_header(store, &policy.retry, object).await {
        Ok(header) => header,
        Err(CommitError::Storage(Error::NotFound { .. })) => return Ok(Removal::AlreadyGone),
        Err(error) => return Err(error),
    };
    if header.kind != ControlObjectKind::Journal {
        debug!(
            object = %name,
            kind = ?header.kind,
            "keeping a head position that is not a Journal record",
        );
        return Ok(Removal::NotARecord);
    }
    match policy.retry.run("trash", || store.trash(object)).await {
        Ok(()) => Ok(Removal::Removed),
        Err(Error::NotFound { .. }) => Ok(Removal::AlreadyGone),
        Err(error) => Err(error.into()),
    }
}

/// Records what the run came to, in generations and counts alone.
fn finished(outcome: &PruneOutcome) {
    let range = outcome.removed_range();
    info!(
        operation = "prune",
        head = outcome.head.get(),
        snapshot = %outcome.snapshot,
        wrote_snapshot = outcome.wrote_snapshot,
        removed = outcome.removed.len(),
        oldest = range.map(|(oldest, _)| oldest.get()),
        newest = range.map(|(_, newest)| newest.get()),
        already_gone = outcome.already_gone.len(),
        kept = outcome.kept.len(),
        unremoved = outcome.unremoved.len(),
        "pruned the Journal records the checkpoint covers",
    );
}
