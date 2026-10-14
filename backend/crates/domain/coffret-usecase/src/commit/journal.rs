use coffret_format::{encode_control_object, encode_journal_record, ControlEncodeRequest};
use coffret_model::{
    ControlObjectKind, ControlObjectName, Generation, IndexCheckpoint, JournalRecord,
    KeyringCommitment, Redacted,
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
use crate::commit_slot::CommitSlot;
use crate::control_head::ControlHead;
use crate::error::Error;
use crate::object_store::ObjectStore;

/// What consuming the commit slot led to.
pub(super) enum Attempted {
    /// The record was created, which is the batch's commit point (spec: CP-1).
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

/// Consumes the current head's commit slot with the batch's Journal record.
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
/// The head is re-read immediately before the create (spec: CP-16). A later
/// epoch's rotation permanently deletes old-epoch control objects, which on a
/// name-keyed Storage frees the key of a slot already consumed; without the
/// re-read a writer that woke long after its epoch ended could create a
/// successor into a position the Library has moved past.
pub(super) async fn commit(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    caught: &CaughtUp,
    standing: Option<&IndexCheckpoint>,
    keyring: KeyringCommitment,
    batch: &PreparedBatch,
) -> CommitResult<Attempted> {
    let (generation, prev) = match caught.head {
        Some(current) => (current.generation().next()?, Some(current.generation())),
        None => (Generation::FIRST, None),
    };
    let head = ControlHead::at(generation);
    let next_commit_slot = head.reserve_commit_slot(store).await?;
    let snapshot_slot = head.reserve_snapshot_slot(store).await?;
    let mints = next_commit_slot.as_provider_id().is_some();

    let commit_slot = match caught.head {
        Some(current) => {
            let Some(slot) = carried_slot(current, standing, mints)? else {
                return Ok(Attempted::Conflict);
            };
            require_head(store, policy, &caught.listing, current).await?;
            slot
        }
        // A Library that has committed nothing has no head to carry a slot, so
        // its first record is created into a slot reserved here, under the name
        // generation 0 takes (spec: FM-13).
        None => {
            store
                .reserve_create(&ControlObjectName::head(Generation::FIRST).to_string())
                .await?
        }
    };

    // The additions arrive in the order the batch spooled them and the removals
    // in the order the Containers they supersede turned up, so the record is put
    // in the Container ID order FM-15 fixes as it is built — and the generation
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
                "committed the batch",
            );
            Ok(Attempted::Committed(Box::new(Committed { record })))
        }
        Err(Error::AlreadyExists { .. }) => {
            // A refusal is a claim that the slot is taken, not proof of it: a
            // create refused because another was in flight, and that one then
            // failing, leaves the slot free (spec: CP-3). Either way this
            // attempt committed nothing and the next one starts from whatever
            // the head turns out to be.
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
