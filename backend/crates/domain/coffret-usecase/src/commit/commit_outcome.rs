use coffret_model::JournalRecord;

use crate::commit::checkpoint_outcome::CheckpointOutcome;
use crate::commit::keyring_repair::KeyringRepair;
use crate::commit::untrashed_removal::UntrashedRemoval;

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
    /// Removed Containers whose objects are still in Storage, and why.
    ///
    /// A removal leaves the current set the moment the record exists; moving
    /// the object to the provider's trash is what happens after, and a device
    /// that could not do it leaves an untrashed removal (spec: OC-6). Reported
    /// so a later run can finish it rather than being lost in a diagnostic
    /// event — the reason along with the Container, because what to do next
    /// differs by which refusal it was.
    pub untrashed: Vec<UntrashedRemoval>,
    /// Every repair this run performed on the committed Keyring (spec: KL-15).
    ///
    /// The one field here that reports work done *before* the commit point, and
    /// it is reported for the reason the two above are: a caller has to be told.
    /// Replica loss and the repair performed are never silent, and a diagnostic
    /// event is not where a person hears about them (spec: EL-1, KL-15).
    ///
    /// Empty where the run rewrote nothing: a committed set examined and found
    /// complete, or a Library with no committed Keyring to examine at all
    /// (spec: FM-13). The two read alike here because they are the same news to
    /// a caller — this run put no replica back.
    ///
    /// One entry per attempt that did put one back, rather than one per run.
    /// An attempt that repaired the set and then lost the commit slot performed
    /// that repair, and the rebase after it examines the set the winner
    /// committed — another generation, which the next entry names (spec: CP-4).
    /// Folding them together would have to pick one of those generations for
    /// work done on both.
    pub repairs: Vec<KeyringRepair>,
}
