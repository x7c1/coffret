use coffret_model::{
    ControlObjectKind, ControlObjectName, Generation, MasterKeyEpoch, ObjectRef, Redacted,
};
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
use crate::prune::unremoved_replica::UnremovedReplica;

/// Removes the Journal records the Snapshot of the current head covers, all
/// but the head itself, and then the Keyring generations that head superseded
/// (spec: CK-4, CK-5, CK-6, CK-8, KL-18).
///
/// The steps, and the rule each answers to, are listed in the
/// [module documentation](crate::prune).
///
/// A refusal is [`PruneError::Refused`](super::PruneError::Refused), and comes
/// before anything is removed; the Snapshot the run made sure of stands either
/// way. Once removing has begun nothing stops it: a record that will not go is
/// reported in [`PruneOutcome::unremoved`], a replica that will not go in
/// [`PruneOutcome::unremoved_replicas`], and the run goes on to the next.
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
        removed_replicas: Vec::new(),
        unremoved_replicas: Vec::new(),
    };
    // At or before what the Snapshot applies, and older than the head, which is
    // never eligible (spec: CK-4). Only names in the head chain are looked at,
    // so no Snapshot, Keyring replica or Container is ever a candidate
    // (spec: CK-6).
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

    // Last, and only once every record the Snapshot covers is gone: a record
    // left in place still selects the generation it committed, and the
    // generation goes after it (spec: KL-18). The next prune takes both.
    match outcome.unremoved.is_empty() {
        true => {
            let recorded = checkpoint.keyring().generation();
            let selected = committed.keyring().generation();
            match superseded_below(checkpoint.master_key_epoch(), recorded, selected) {
                Some(bound) => dispose_of_superseded(store, &policy, &listing, bound, &mut outcome).await,
                None => info!(
                    epoch = checkpoint.master_key_epoch().get(),
                    "the Library is past its first Master Key epoch; no Keyring generation is disposed of",
                ),
            }
        }
        false => info!(
            unremoved = outcome.unremoved.len(),
            "Journal records were left in place; the superseded Keyring generations wait for the next prune",
        ),
    }

    finished(&outcome);
    Ok(outcome)
}

/// The generation below which every listed Keyring replica belongs to a
/// generation of the current epoch that a later one superseded, or `None`
/// where the run cannot tell which older generations are the epoch's own
/// (spec: KL-10, KL-18).
///
/// Below both the generation the Snapshot records and the one the head selects
/// — one and the same, since the gate has just shown the Snapshot records the
/// head's tuple (spec: CK-5), but each is a generation KL-18 never disposes of,
/// so both bound it. Every generation below is superseded: a later one, the
/// head's, is committed and complete (spec: KL-2, KL-3).
///
/// Only in the first epoch. A generation's number determines its epoch and
/// never restarts (spec: KL-10), so in the first epoch every older generation
/// is the epoch's own; past a rotation the older numbers include the previous
/// epochs' generations, whose replicas no name tells apart from the current
/// epoch's, and rotation — which deletes those outright (spec: MR-3) — is not
/// implemented. A Library past its first epoch keeps every generation.
///
/// Every set listed under an older number goes, whatever its digest: a
/// candidate that lost its commit slot at that number is no more reachable
/// than the committed set beside it, and which digest the records committed is
/// nothing the names say (spec: KL-14).
fn superseded_below(
    epoch: MasterKeyEpoch,
    recorded: Generation,
    selected: Generation,
) -> Option<Generation> {
    (epoch == MasterKeyEpoch::FIRST).then_some(recorded.min(selected))
}

/// Trashes every replica of every Keyring generation below `bound`
/// (spec: KL-18).
///
/// Trash and not purge, for the reason a record is trashed: only a Master Key
/// rotation deletes Keyring replicas irreversibly (spec: MR-3). A replica that
/// will not go is reported and the run goes on to the next.
async fn dispose_of_superseded(
    store: &dyn ObjectStore,
    policy: &CommitPolicy,
    listing: &ControlListing,
    bound: Generation,
    outcome: &mut PruneOutcome,
) {
    for (replica, copies) in listing.keyring_replicas() {
        if replica.generation() >= bound {
            continue;
        }
        match trash_copies(store, policy, copies).await {
            Ok(true) => outcome.removed_replicas.push(replica.clone()),
            Ok(false) => debug!(
                object = %replica,
                "a superseded Keyring replica was already gone",
            ),
            Err(cause) => {
                warn!(
                    object = %replica,
                    reason = %cause.redacted(),
                    "a superseded Keyring replica was not trashed; the next prune takes it",
                );
                outcome.unremoved_replicas.push(UnremovedReplica {
                    replica: replica.clone(),
                    cause,
                });
            }
        }
    }
}

/// Trashes every object listed under one replica name, and says whether any
/// was there to trash.
///
/// Every copy, because on a Storage whose names are not unique a name can
/// carry several (spec: KL-14), and a copy left behind is a replica left
/// behind. One already gone is where the run was taking it; the first failure
/// is what the replica is reported with, after the rest have been tried.
async fn trash_copies(
    store: &dyn ObjectStore,
    policy: &CommitPolicy,
    copies: &[ObjectRef],
) -> Result<bool, CommitError> {
    let mut trashed = false;
    let mut failed = None;
    for object in copies {
        match policy.retry.run("trash", || store.trash(object)).await {
            Ok(()) => trashed = true,
            Err(Error::NotFound { .. }) => {}
            Err(error) => {
                failed.get_or_insert(CommitError::from(error));
            }
        }
    }
    match failed {
        Some(cause) => Err(cause),
        None => Ok(trashed),
    }
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
        removed_replicas = outcome.removed_replicas.len(),
        unremoved_replicas = outcome.unremoved_replicas.len(),
        "pruned the Journal records the checkpoint covers and the Keyring generations it superseded",
    );
}
