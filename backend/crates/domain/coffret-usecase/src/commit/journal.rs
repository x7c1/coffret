use std::collections::BTreeSet;

use coffret_format::{encode_control_object, encode_journal_record, ControlEncodeRequest};
use coffret_model::{
    ContainerId, ControlObjectKind, ControlObjectName, EntryRename, Generation, IndexCheckpoint,
    JournalRecord, KeyringCommitment, Redacted,
};
use tracing::{debug, info, warn};

use crate::byte_stream::ByteStream;
use crate::commit::catch_up::CaughtUp;
use crate::commit::commit_error::{CommitError, CommitResult, ControlObjectFault};
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::control_object;
use crate::commit::prepared_batch::PreparedBatch;
use crate::commit::slot_occupant;
use crate::commit_slot::CommitSlot;
use crate::control_head::ControlHead;
use crate::device_state::AttemptedSlot;
use crate::error::Error;
use crate::object_store::ObjectStore;

/// What consuming the commit slot led to.
pub(super) enum Attempted {
    /// The record was created, which is the batch's commit point (spec: CP-1)
    /// — or the create was refused and the slot read back holds the batch's
    /// own record, the answer to the create having been lost (spec: CP-3).
    Committed(Box<Committed>),
    /// The slot is taken and holds a successor this device did not write, so
    /// this attempt has committed nothing (spec: CP-3, CP-4).
    Conflict,
}

/// A batch that has committed.
pub(super) struct Committed {
    /// The record that committed it.
    pub(super) record: JournalRecord,
}

/// The slot one attempt is about to consume, and the two its record will carry.
///
/// Deciding it is split from creating into it so that the batch can durably
/// record which slot it attempts before the create can reach Storage
/// (spec: OC-2): the slot is what a later settle reads back to prove the batch
/// did not commit there (spec: OC-3).
pub(super) struct Aim {
    /// The generation the record takes, `head-<generation>` being its name.
    generation: Generation,
    /// The head it succeeds, or `None` for a Library's first record.
    prev: Option<Generation>,
    /// The slot the record is created into.
    commit_slot: CommitSlot,
    /// The slot reserved for the record's own successor (spec: CP-2).
    next_commit_slot: CommitSlot,
    /// The slot reserved for the record's ordinary checkpoint (spec: CK-10).
    snapshot_slot: CommitSlot,
}

impl Aim {
    /// What the batch records before the create (spec: OC-2).
    pub(super) fn attempted(&self) -> AttemptedSlot {
        AttemptedSlot::consuming(self.generation, &self.commit_slot)
    }
}

/// Decides the slot this attempt consumes: the one the current head carries.
///
/// The slot consumed is the one the head carries (spec: CP-2): of the writers
/// that start from one head exactly one succeeds because every one of them aims
/// at that single slot (spec: CP-3). Where the Storage mints identifiers it is
/// the pre-minted identifier in the head's `next_commit_slot`; where names
/// identify objects the head persists nothing and the slot is the name
/// `head-<generation + 1>`, re-derived here (spec: CP-15). What the head
/// carries is read off the Index, which the catch-up just brought to that head
/// — the replay or the adopted Snapshot put the head's own `next_commit_slot`
/// there (spec: CK-2) — so the head is not fetched a second time for it.
///
/// Two slots are still reserved afresh, because they are what the new record
/// itself carries and the record has to name both before it is sealed: the one
/// its own successor will take, and the one its ordinary checkpoint goes in
/// (spec: CK-10). What a record persists for those is Storage's own opaque
/// token and nothing else — nothing at all where the provider mints none,
/// because there the name is re-derived when the slot is consumed and two
/// spellings could not then drift apart (spec: CP-2, CP-15). The fresh
/// reservation is also what says which kind of Storage this is, so a head whose
/// slot is not in that Storage's form is refused rather than consumed
/// (see [`carried_slot`]).
///
/// The head is re-read here, immediately before the create (spec: CP-16):
/// nothing but recording the slot comes between this and [`commit`]. A later
/// epoch's rotation permanently deletes old-epoch control objects, which on a
/// name-keyed Storage frees the key of a slot already consumed; without the
/// re-read a writer that woke long after its epoch ended could create a
/// successor into a position the Library has moved past.
///
/// A Library with no head has none to carry a slot, so the aim is the one
/// [`first_aim`] decides for `head-0` — which the commit flow reaches only
/// with an empty batch, the empty first head itself (spec: CP-2, FM-13).
///
/// `None` says this attempt has no slot to consume and the next one catches up
/// afresh (spec: CP-4). Nothing has been created either way.
pub(super) async fn aim(
    store: &dyn ObjectStore,
    policy: &CommitPolicy,
    caught: &CaughtUp,
    standing: Option<&IndexCheckpoint>,
) -> CommitResult<Option<Aim>> {
    let Some(current) = caught.head else {
        return first_aim(store).await.map(Some);
    };
    let generation = current.generation().next()?;
    let head = ControlHead::at(generation);
    let next_commit_slot = head.reserve_commit_slot(store).await?;
    let snapshot_slot = head.reserve_snapshot_slot(store).await?;
    let mints = next_commit_slot.as_provider_id().is_some();

    let Some(commit_slot) = carried_slot(current, standing, mints)? else {
        return Ok(None);
    };
    require_head(store, policy, &caught.listing, current).await?;

    Ok(Some(Aim {
        generation,
        prev: Some(current.generation()),
        commit_slot,
        next_commit_slot,
        snapshot_slot,
    }))
}

/// Decides the slot a Library's first head is created into (spec: CP-2,
/// FM-13).
///
/// A Library with no head has nothing to carry a slot, so the slot generation
/// 0 takes is reserved here, under the name `head-0`, by the writer that is
/// about to consume it. That is the one reservation a writer ever makes for
/// itself. Creating a Library makes it before any other device knows the
/// Library exists, so nothing races it there. A Library created before its
/// first head was written at creation has it made by its first commit, which
/// writes an empty `head-0` this way before committing its batch on top of it;
/// two such commits can race here (spec: CP-2). Where Storage mints
/// identifiers the reservation is this writer's own, so what it records of the
/// slot (spec: OC-2) can only ever read back as its own record or as nothing.
pub(super) async fn first_aim(store: &dyn ObjectStore) -> CommitResult<Aim> {
    let head = ControlHead::at(Generation::FIRST);
    let next_commit_slot = head.reserve_commit_slot(store).await?;
    let snapshot_slot = head.reserve_snapshot_slot(store).await?;
    let commit_slot = store
        .reserve_create(&ControlObjectName::head(Generation::FIRST).to_string())
        .await?;
    Ok(Aim {
        generation: Generation::FIRST,
        prev: None,
        commit_slot,
        next_commit_slot,
        snapshot_slot,
    })
}

/// Consumes the slot [`aim`] decided with the batch's Journal record.
///
/// The batch has recorded the slot by now (spec: OC-2), so whatever becomes of
/// the create, a later settle knows where to look for it (spec: OC-3).
pub(super) async fn commit(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    aim: Aim,
    keyring: KeyringCommitment,
    batch: &PreparedBatch,
) -> CommitResult<Attempted> {
    let Aim {
        generation,
        prev,
        commit_slot,
        next_commit_slot,
        snapshot_slot,
    } = aim;

    // The additions arrive in the order the batch spooled them and the removals
    // in the order the Containers they supersede turned up, so the record is put
    // in the Container ID order FM-15 fixes as it is built, the renames in the
    // order of the Entries they name — and the generation
    // and the head it succeeds, computed above from the head this commit is
    // rebasing on, are confirmed to agree there rather than here (spec: FM-15).
    let record = JournalRecord::canonical(
        generation,
        prev,
        keys.master_key_epoch(),
        keyring,
        next_commit_slot.as_provider_id().map(str::to_owned),
        snapshot_slot.as_provider_id().map(str::to_owned),
        batch
            .additions
            .iter()
            .map(|prepared| prepared.addition.clone())
            .collect(),
        batch.removals.clone(),
    )
    .and_then(|record| record.with_renames(renames(batch)?))
    .map_err(|cause| CommitError::UnwritableControlValue { cause })?;
    let name = ControlObjectName::head(generation);
    let payload = encode_journal_record(&record)?;
    let object = encode_control_object(&ControlEncodeRequest::new(
        &name,
        ControlObjectKind::Journal,
        keys.of_kind(ControlObjectKind::Journal),
        &payload,
    ))?;

    match policy
        .retry
        .run("put_if_absent", || {
            store.put_if_absent(&commit_slot, ByteStream::from(object.bytes().to_vec()))
        })
        .await
    {
        Ok(_) => {
            info!(
                object = %name,
                generation = generation.get(),
                additions = record.additions().len(),
                removals = record.removals().len(),
                renames = record.renames().len(),
                "committed the batch",
            );
            Ok(Attempted::Committed(Box::new(Committed { record })))
        }
        Err(Error::AlreadyExists { .. }) => {
            // The refusal may be this run's own create answering back, its
            // answer having been lost: `own_record` recognises the batch's own
            // record in the slot (spec: CP-3).
            //
            // A batch that adds nothing (removals or renames only) has no
            // Container of its own for a record to name, and nothing else in a
            // record is the batch's alone: another writer may remove or rename
            // the same things. So it is not recognised and stays a conflict;
            // its rebase then refuses it against the record it finds.
            let additions: BTreeSet<ContainerId> = batch
                .additions
                .iter()
                .map(|prepared| prepared.addition.container().id)
                .collect();
            if !additions.is_empty() {
                let slot = AttemptedSlot::consuming(generation, &commit_slot);
                if let Some(record) =
                    slot_occupant::own_record(store, &policy.retry, keys, &slot, &additions).await
                {
                    info!(
                        object = %name,
                        generation = generation.get(),
                        additions = record.additions().len(),
                        removals = record.removals().len(),
                        renames = record.renames().len(),
                        "committed the batch; the commit slot holds its record after the \
                         answer to its create was lost",
                    );
                    return Ok(Attempted::Committed(Box::new(Committed { record })));
                }
            }
            // Otherwise a refusal is a claim that the slot is taken, not proof
            // of it: a create refused because another was in flight, and that
            // one then failing, leaves the slot free (spec: CP-3). Either way
            // this attempt committed nothing and the next one starts from
            // whatever the head turns out to be.
            debug!(
                object = %name,
                generation = generation.get(),
                "the commit slot was refused; rebasing onto the new head",
            );
            Ok(Attempted::Conflict)
        }
        Err(error) => Err(error.into()),
    }
}

/// The batch's renames as its record carries them: in Container ID and then
/// entry number order (spec: FM-15).
fn renames(batch: &PreparedBatch) -> coffret_model::Result<Vec<EntryRename>> {
    let mut renames = batch
        .renames
        .iter()
        .map(|rename| {
            EntryRename::new(
                rename.entry.container_id,
                rename.entry.entry_number,
                rename.path.clone(),
            )
        })
        .collect::<coffret_model::Result<Vec<_>>>()?;
    renames.sort_by_key(EntryRename::order_key);
    Ok(renames)
}

/// The commit slot the head this commit succeeds carries, in the form this
/// Storage consumes it (spec: CP-2, CP-15).
///
/// `standing` is where the Index stands once the catch-up has brought it to the
/// head, and its `next_commit_slot` is the head's own. `mints` says whether
/// this Storage mints identifiers, which the fresh reservation the new record
/// carries has just shown.
///
/// `None` says the Index no longer stands at this head, and so neither does the
/// Library: another process sharing the catalog has replayed past it, or —
/// where nothing explains it — the catalog is not where the catch-up left it.
/// Either way this attempt has no slot to consume and the next one catches up
/// afresh (spec: CP-4).
///
/// A head whose slot is not in this Storage's form is refused, and nothing is
/// put in its place. One that carries no identifier on a Storage that mints
/// them is the hole CP-3 closes: a slot minted here would be this writer's
/// alone, and another writer starting from the same head would succeed it too.
/// One that carries an identifier on a Storage that keys objects by name names
/// a slot that Storage cannot create under (spec: CP-2).
fn carried_slot(
    current: ControlHead,
    standing: Option<&IndexCheckpoint>,
    mints: bool,
) -> CommitResult<Option<CommitSlot>> {
    let generation = current.generation();
    let standing = match standing {
        Some(standing) if standing.head_generation() == generation => standing,
        other => {
            let stands_at = other.map(|checkpoint| checkpoint.head_generation().get());
            if other.is_some_and(|checkpoint| checkpoint.head_generation() > generation) {
                debug!(
                    head = generation.get(),
                    stands_at, "the catalog moved past the head this commit caught up to; rebasing",
                );
            } else {
                warn!(
                    head = generation.get(),
                    stands_at,
                    "the catalog does not stand at the head this commit caught up to; rebasing",
                );
            }
            return Ok(None);
        }
    };

    let fault = match (standing.next_commit_slot(), mints) {
        (Some(_), true) | (None, false) => {
            return Ok(Some(current.commit_slot(standing.next_commit_slot())?))
        }
        (None, true) => ControlObjectFault::NoCommitSlot,
        (Some(_), false) => ControlObjectFault::MintedCommitSlot,
    };
    warn!(
        head = generation.get(),
        reason = %fault.redacted(),
        "refusing to commit onto a head whose commit slot this Storage cannot consume",
    );
    Err(CommitError::CorruptControlObject {
        object: ControlObjectName::head(generation),
        fault,
    })
}

/// Re-reads the head the commit slot came from, and refuses if it is gone
/// (spec: CP-16).
///
/// Only the authenticated header is fetched. That is all the rule needs — it
/// asks whether the head is still there and still the head it was — and it
/// spares a writer from pulling a whole Index Snapshot back down to find out.
async fn require_head(
    store: &dyn ObjectStore,
    policy: &CommitPolicy,
    listing: &ControlListing,
    head: ControlHead,
) -> CommitResult<()> {
    let generation = head.generation();
    let name = ControlObjectName::head(generation);
    let object = listing
        .handle(&name.to_string())
        .ok_or(CommitError::MissingHead { generation })?;

    let header = match control_object::fetch_header(store, &policy.retry, object).await {
        Ok(header) => header,
        Err(CommitError::Storage(Error::NotFound { .. })) => {
            return Err(CommitError::MissingHead { generation })
        }
        Err(error) => return Err(error),
    };
    if header.generation != generation {
        return Err(CommitError::CorruptControlObject {
            object: name,
            fault: ControlObjectFault::GenerationMismatch {
                found: header.generation,
            },
        });
    }
    Ok(())
}
