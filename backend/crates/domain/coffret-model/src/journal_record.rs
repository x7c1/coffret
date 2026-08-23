use crate::container_addition::ContainerAddition;
use crate::container_id::ContainerId;
use crate::generation::Generation;
use crate::index_checkpoint::IndexCheckpoint;
use crate::keyring_commitment::KeyringCommitment;
use crate::master_key_epoch::MasterKeyEpoch;

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
    /// The generation of the control head this record succeeds, and `None` at
    /// generation 0, where the Library has no earlier head (spec: FM-13, FM-15).
    ///
    /// Which kind that head was is not stated here: whichever kind won its
    /// commit slot took the position, and both are equally this record's
    /// predecessor (spec: CP-2, CP-6).
    pub prev: Option<Generation>,
    /// The Master Key epoch this record belongs to (spec: CP-13, FM-13).
    pub master_key_epoch: MasterKeyEpoch,
    /// The exact Keyring replica set this commit selects (spec: CP-10, KL-3).
    pub keyring: KeyringCommitment,
    /// The slot this record's own successor is committed into, as Storage's
    /// opaque token, and `None` where the provider mints none (spec: CP-2).
    pub next_commit_slot: Option<String>,
    /// The one slot this head's ordinary Index Snapshot may be created into,
    /// reserved by this record's writer before the commit and in the same form
    /// as the commit slot (spec: CK-10, CP-2).
    ///
    /// `None` where the provider mints no identifier: there the slot is the
    /// Snapshot's name, re-derived from this record's generation at spend time
    /// rather than persisted, so the two spellings cannot drift apart
    /// (spec: CP-15).
    pub snapshot_slot: Option<String>,
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
