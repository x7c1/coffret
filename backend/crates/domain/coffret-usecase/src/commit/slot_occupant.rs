use std::collections::BTreeSet;

use coffret_format::decode_journal_record;
use coffret_model::{ContainerId, ControlObjectKind, JournalRecord, Redacted};
use tracing::{debug, warn};

use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::control_object;
use crate::device_state::AttemptedSlot;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// What a commit slot a batch recorded holds now, as far as settling the batch
/// needs to know (spec: OC-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SlotOccupant {
    /// Nothing that proves anything: the slot is empty, its object could not
    /// be read or authenticated, or it is gone — pruned, or deleted with its
    /// epoch (spec: OC-3).
    Unproven,
    /// An authenticated Journal record, with the Containers it adds.
    ///
    /// One that names a Container of the batch is the batch's own commit
    /// (spec: OC-7); one that names none of them is a different writer's,
    /// whichever device wrote it, because a Container ID is never reused
    /// (spec: CP-14).
    Journal {
        /// The Containers the record adds.
        additions: BTreeSet<ContainerId>,
    },
    /// An authenticated Index Snapshot that activated an epoch: a successor
    /// that is not a commit at all (spec: CP-5).
    Activation,
}

impl SlotOccupant {
    /// Whether the slot holds a record naming `container_id` among its
    /// additions: the batch committed there (spec: OC-3, OC-7).
    pub(crate) fn names(&self, container_id: ContainerId) -> bool {
        matches!(self, Self::Journal { additions } if additions.contains(&container_id))
    }

    /// Whether the slot holds an authenticated successor that does not name
    /// `container_id`: proof that the create of the batch it belongs to did not
    /// land there (spec: OC-3).
    pub(crate) fn excludes(&self, container_id: ContainerId) -> bool {
        match self {
            Self::Unproven => false,
            Self::Journal { additions } => !additions.contains(&container_id),
            Self::Activation => true,
        }
    }
}

/// Reads back what one recorded commit slot holds (spec: OC-3).
///
/// The slot is reached the way the create aimed at it, through
/// [`object_at`](ObjectStore::object_at), never by name alone: on a Storage that
/// mints identifiers a name may stand for more than one object (spec: CP-2).
/// It must also be what the listing the run already walked holds under that
/// name. An object the walk does not list is gone from the Library — pruned, or
/// deleted with its epoch — even where the provider would still hand its bytes
/// back out of a trash, and one listed under the name as some other object is a
/// slot this walk cannot vouch for. Neither is proof.
///
/// Every failure is read as no proof rather than raised: a settle that cannot
/// prove a batch abandoned retains it, and the next run asks again.
pub(crate) async fn read(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &ControlKeys,
    listing: &ControlListing,
    slot: &AttemptedSlot,
) -> SlotOccupant {
    match occupant(store, retry, keys, listing, slot).await {
        Ok(occupant) => occupant,
        Err(error) => {
            warn!(
                generation = slot.generation().get(),
                reason = %error.redacted(),
                "a commit slot a batch attempted could not be read back; it proves nothing",
            );
            SlotOccupant::Unproven
        }
    }
}

/// Reads back the record at the slot this run's own create was just refused
/// at, where it is the batch's own (spec: CP-3, OC-3).
///
/// A create whose object landed and whose answer was lost is retried, and the
/// retry is refused because the slot holds what the first try wrote. A record
/// there that names exactly `additions` is that object: a Container ID is never
/// reused (spec: CP-14), so no other writer's record names this batch's
/// Containers, which is the proof OC-3 already takes for a batch having
/// committed at a slot. `additions` must not be empty; a batch that adds
/// nothing leaves nothing of its own to recognise.
///
/// The listing is walked afresh, since the one the run caught up from predates
/// the create. Every failure, and a record that names anything else, is read
/// as no proof — `None` — and the caller treats the slot as taken by another
/// writer, which is what it did before it asked.
pub(crate) async fn own_record(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &ControlKeys,
    slot: &AttemptedSlot,
    additions: &BTreeSet<ContainerId>,
) -> Option<JournalRecord> {
    let read = async {
        let listing = ControlListing::read(store, retry).await?;
        read_back(store, retry, keys, &listing, slot).await
    };
    match read.await {
        Ok(ReadBack::Journal(record)) if added(&record) == *additions => Some(record),
        Ok(_) => None,
        Err(error) => {
            warn!(
                generation = slot.generation().get(),
                reason = %error.redacted(),
                "the commit slot this run was refused at could not be read back; \
                 treating it as taken by another writer",
            );
            None
        }
    }
}

async fn occupant(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &ControlKeys,
    listing: &ControlListing,
    slot: &AttemptedSlot,
) -> CommitResult<SlotOccupant> {
    Ok(match read_back(store, retry, keys, listing, slot).await? {
        ReadBack::Unproven => SlotOccupant::Unproven,
        ReadBack::Journal(record) => SlotOccupant::Journal {
            additions: added(&record),
        },
        ReadBack::Activation => SlotOccupant::Activation,
    })
}

/// What an authenticated read of one commit slot found.
enum ReadBack {
    /// Nothing that proves anything (see [`SlotOccupant::Unproven`]).
    Unproven,
    /// An authenticated Journal record.
    Journal(JournalRecord),
    /// An authenticated Index Snapshot that activated an epoch.
    Activation,
}

/// Reads one commit slot through the listing, authenticated and decoded
/// (see [`read`]).
async fn read_back(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &ControlKeys,
    listing: &ControlListing,
    slot: &AttemptedSlot,
) -> CommitResult<ReadBack> {
    let name = slot.name();
    let object = store.object_at(&slot.commit_slot())?;
    if listing.handle(&name.to_string()) != Some(&object) {
        debug!(
            generation = slot.generation().get(),
            "a commit slot a batch attempted holds nothing the listing vouches for",
        );
        return Ok(ReadBack::Unproven);
    }
    let decoded = match control_object::read(store, retry, keys, &name, &object).await {
        Ok(decoded) => decoded,
        // Listed a moment ago and gone now: another device is pruning.
        Err(CommitError::Storage(crate::Error::NotFound { .. })) => return Ok(ReadBack::Unproven),
        Err(error) => return Err(error),
    };
    Ok(match decoded.kind {
        ControlObjectKind::Journal => {
            ReadBack::Journal(decode_journal_record(&decoded.payload, slot.generation())?)
        }
        ControlObjectKind::ActivationSnapshot => ReadBack::Activation,
        // The framing admits neither under a head's name (spec: FM-12), so the
        // read above has already refused them; nothing here proves anything.
        ControlObjectKind::IndexSnapshot | ControlObjectKind::Keyring => ReadBack::Unproven,
    })
}

/// The Containers a record adds.
fn added(record: &JournalRecord) -> BTreeSet<ContainerId> {
    record
        .additions()
        .iter()
        .map(|addition| addition.container().id)
        .collect()
}
