use coffret_format::{encode_control_object, encode_journal_record, ControlEncodeRequest};
use coffret_model::{
    ControlObjectKind, ControlObjectName, Generation, JournalRecord, KeyringCommitment,
};
use tracing::{debug, info};

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

/// What spending the commit slot led to.
pub(super) enum Attempted {
    /// The record was created, which is the batch's commit point (spec: CP-1).
    Committed(Box<Committed>),
    /// The slot is taken and holds a successor this device did not write, so
    /// this attempt has committed nothing (spec: CP-3, CP-4).
    Conflict,
}

/// A batch that has committed, and the one reservation the commit leaves behind.
pub(super) struct Committed {
    /// The record that committed it.
    pub(super) record: JournalRecord,
    /// The slot this head's ordinary Index Snapshot may be created into, which
    /// the record already names (spec: CK-10).
    pub(super) snapshot_slot: CommitSlot,
}

/// Spends the current head's commit slot on the batch's Journal record.
///
/// Three slots are reserved, and they are three because a head hands out two of
/// them and the record has to name both of its own before it is sealed: the one
/// this record is created into (spec: CP-2), the one its own successor will
/// take, and the one its ordinary checkpoint goes in (spec: CK-10). What a
/// record persists for the latter two is Storage's own opaque token and nothing
/// else — nothing at all where the provider mints none, because there the name
/// is re-derived at spend time and two spellings could not then drift apart
/// (spec: CP-2, CP-15).
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
    keyring: KeyringCommitment,
    batch: &PreparedBatch,
) -> CommitResult<Attempted> {
    let (generation, prev, commit_slot) = reserve_commit(store, caught).await?;
    let head = ControlHead::at(generation);
    let next_commit_slot = head.reserve_commit_slot(store).await?;
    let snapshot_slot = head.reserve_snapshot_slot(store).await?;

    if let Some(current) = caught.head {
        require_head(store, policy, &caught.listing, current).await?;
    }

    // The additions arrive in the order the batch spooled them and the removals
    // in the order the Containers they displace turned up, so the record is put
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
            Ok(Attempted::Committed(Box::new(Committed {
                record,
                snapshot_slot,
            })))
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

/// The generation this commit takes, the head it succeeds, and the slot it is
/// created into (spec: CP-2, FM-13).
///
/// A Library that has committed nothing has no head to succeed: its first
/// record is written as generation 0 and states no predecessor, so the slot is
/// that name rather than a successor derived from one.
async fn reserve_commit(
    store: &dyn ObjectStore,
    caught: &CaughtUp,
) -> CommitResult<(Generation, Option<Generation>, CommitSlot)> {
    match caught.head {
        Some(head) => Ok((
            head.generation().next()?,
            Some(head.generation()),
            head.reserve_commit_slot(store).await?,
        )),
        None => {
            let name = ControlObjectName::head(Generation::FIRST);
            let slot = store.reserve_create(&name.to_string()).await?;
            Ok((Generation::FIRST, None, slot))
        }
    }
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
