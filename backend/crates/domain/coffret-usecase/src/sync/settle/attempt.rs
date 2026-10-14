use std::collections::{BTreeMap, BTreeSet};

use coffret_model::ContainerId;

use super::complete::completes;
use super::SettleStorage;
use crate::commit::{read_slot_occupant, CommitPolicy, SlotOccupant};
use crate::device_state::{BatchId, PendingRow};
use crate::index::Index;
use crate::sync::sync_error::SyncResult;

/// What the slots a batch recorded say about one of its Containers.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Attempt {
    /// A record at one of them names the Container: the batch committed there
    /// (spec: OC-7).
    Committed,
    /// Every one of them holds an authenticated successor that does not name
    /// it: the batch lost every slot it attempted, and its create landed in
    /// none of them (spec: OC-3, CP-18).
    Lost,
    /// Neither: no slot recorded, or one that proves nothing (spec: OC-2, OC-3).
    Unknown,
}

/// Judges one attempted row by what its batch's recorded slots hold.
///
/// Every slot is needed, because the create may have landed in any one the
/// batch attempted; and a batch that recorded none — written before slots were
/// recorded at all — is missing the evidence, which OC-2 reads as uncertain.
pub(super) fn judge(container_id: ContainerId, occupants: &[SlotOccupant]) -> Attempt {
    if occupants
        .iter()
        .any(|occupant| occupant.names(container_id))
    {
        Attempt::Committed
    } else if !occupants.is_empty()
        && occupants
            .iter()
            .all(|occupant| occupant.excludes(container_id))
    {
        Attempt::Lost
    } else {
        Attempt::Unknown
    }
}

/// Reads back every slot each batch recorded, once per batch, for the batches
/// with an attempted row left to judge (spec: OC-3).
///
/// A row that completes as current needs none of it, so a batch is read only
/// for a row the current set says nothing about.
pub(super) async fn attempts(
    storage: &SettleStorage<'_>,
    index: &dyn Index,
    policy: &CommitPolicy,
    pending: &[PendingRow],
    current: &BTreeSet<ContainerId>,
) -> SyncResult<BTreeMap<BatchId, Vec<SlotOccupant>>> {
    let batches: BTreeSet<&BatchId> = pending
        .iter()
        .filter(|row| row.commit_attempted && !completes(row, current))
        .map(|row| &row.batch)
        .collect();
    let mut attempts = BTreeMap::new();
    for batch in batches {
        let slots = index.attempted_slots(batch).await?;
        let mut occupants = Vec::with_capacity(slots.len());
        for slot in &slots {
            occupants.push(
                read_slot_occupant(
                    storage.store,
                    &policy.retry,
                    storage.keys,
                    &storage.caught.listing,
                    slot,
                )
                .await,
            );
        }
        attempts.insert(batch.clone(), occupants);
    }
    Ok(attempts)
}
