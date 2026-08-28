use coffret_model::{ContainerId, JournalRecord};

use crate::commit::checkpoint_outcome::CheckpointOutcome;

/// What a successful commit did.
///
/// The record is the commit point and everything else here happened after it
/// (spec: CP-1), which is why two of these fields report work that may not have
/// finished: trashing a removed Container and writing a checkpoint are both
/// retryable afterwards and neither can un-commit the batch.
#[derive(Debug)]
pub struct CommitOutcome {
    /// The record that committed the batch (spec: CP-1).
    pub record: JournalRecord,
    /// How many attempts it took, each one after the first a rebase onto a head
    /// another writer committed (spec: CP-4).
    pub attempts: u32,
    /// What the checkpoint policy did (spec: CK-8).
    pub checkpoint: CheckpointOutcome,
    /// Removed Containers whose objects are still in Storage.
    ///
    /// A removal leaves the current set the moment the record exists; moving
    /// the object to the provider's trash is what happens after, and a device
    /// that could not do it leaves an untrashed removal (spec: OC-6). Reported
    /// so a later run can finish it rather than being lost in a log line.
    pub untrashed: Vec<ContainerId>,
}
