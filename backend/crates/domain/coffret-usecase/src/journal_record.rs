use coffret_model::{ContainerId, Generation, IndexCheckpoint, KeyringCommitment, MasterKeyEpoch};

use crate::container_addition::ContainerAddition;

/// One committed Journal record, as the Index replays it.
///
/// The record is the commit point of a batch: before it exists the batch has
/// changed nothing, and once it exists its additions and removals are part of
/// the current Container set, never partially (spec: CP-1). Replaying it is
/// therefore the whole of what an Index has to do to catch up one step, and it
/// opens no Container to do so, because the additions carry what the Containers
/// they name hold (spec: CP-11, CK-9).
///
/// This is the record's content as a domain value. How it is encoded, encrypted
/// under a purpose key, and framed as a control object is the format layer's
/// business (spec: CP-12, FM-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRecord {
    /// The head-chain generation this record becomes on committing
    /// (spec: CP-2, FM-13).
    pub generation: Generation,
    /// The Master Key epoch this record belongs to (spec: CP-13, FM-13).
    pub master_key_epoch: MasterKeyEpoch,
    /// The exact Keyring replica set this commit selects (spec: CP-10, KL-3).
    pub keyring: KeyringCommitment,
    /// The slot this record's own successor is committed into, as Storage's
    /// opaque token, and `None` where the provider mints none (spec: CP-2).
    pub next_commit_slot: Option<String>,
    /// The Containers the batch added, with their entry tables (spec: CP-11).
    pub additions: Vec<ContainerAddition>,
    /// The Containers the batch removed.
    ///
    /// A removed Container ID is never added again, so removal from the current
    /// set is monotonic and replaying a record twice removes the same thing
    /// twice (spec: CP-14).
    pub removals: Vec<ContainerId>,
}

impl JournalRecord {
    /// The checkpoint an Index stands at once this record is applied.
    ///
    /// A Journal record is both a head and the last Journal generation applied
    /// to reach it, so the two generations coincide here; they diverge only at
    /// an epoch activation, whose Snapshot takes a head position without being
    /// a Journal record (spec: CK-1, CP-6).
    pub fn checkpoint(&self) -> IndexCheckpoint {
        IndexCheckpoint {
            master_key_epoch: self.master_key_epoch,
            head_generation: self.generation,
            journal_generation: self.generation,
            next_commit_slot: self.next_commit_slot.clone(),
            keyring: self.keyring.clone(),
        }
    }
}
