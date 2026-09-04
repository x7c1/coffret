use crate::container_addition::ContainerAddition;
use crate::container_id::ContainerId;
use crate::generation::Generation;
use crate::index_checkpoint::IndexCheckpoint;
use crate::keyring_commitment::KeyringCommitment;
use crate::master_key_epoch::MasterKeyEpoch;

mod new;

#[cfg(test)]
mod tests;

/// One committed Journal record, as the Index replays it.
///
/// The record is the commit point of a batch: before it exists the batch has
/// changed nothing, and once it exists its additions and removals are part of
/// the current Container set, never partially (spec: CP-1). Replaying it is
/// therefore the whole of what an Index has to do to catch up one step, and it
/// opens no Container to do so, because the additions carry what the Containers
/// they name hold (spec: CP-11, CK-9).
///
/// Two rules FM-15 states are held here rather than at each reader: the record
/// names the head it succeeds, and its two collections are in Container ID
/// order. A device replaying one of these therefore follows the head chain and
/// re-encodes the same bytes without checking either again. Both are
/// [`new`](Self::new)'s to state, so everything this type answers below answers
/// without a refusal to report.
///
/// This is the record's content as a domain value. How it is encoded, encrypted
/// under a purpose key, and framed as a control object is the format layer's
/// business (spec: CP-12, FM-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRecord {
    generation: Generation,
    prev: Option<Generation>,
    master_key_epoch: MasterKeyEpoch,
    keyring: KeyringCommitment,
    next_commit_slot: Option<String>,
    snapshot_slot: Option<String>,
    additions: Vec<ContainerAddition>,
    removals: Vec<ContainerId>,
}

impl JournalRecord {
    /// The checkpoint an Index stands at once this record is applied.
    ///
    /// A Journal record is both a head and the last Journal generation applied
    /// to reach it, so the two generations coincide here; they diverge only at
    /// an epoch activation, whose Snapshot takes a head position without being
    /// a Journal record (spec: CK-1, CP-6).
    pub fn checkpoint(&self) -> IndexCheckpoint {
        IndexCheckpoint::at_head(
            self.master_key_epoch,
            self.generation,
            self.next_commit_slot.clone(),
            self.keyring.clone(),
        )
    }

    /// The head-chain generation this record becomes on committing
    /// (spec: CP-2, FM-13).
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// The generation of the control head this record succeeds, and `None` at
    /// generation 0, where the Library has no earlier head (spec: FM-13,
    /// FM-15).
    ///
    /// Which kind that head was is not stated here: whichever kind won its
    /// commit slot took the position, and both are equally this record's
    /// predecessor (spec: CP-2, CP-6).
    pub const fn prev(&self) -> Option<Generation> {
        self.prev
    }

    /// The Master Key epoch this record belongs to (spec: CP-13, FM-13).
    pub const fn master_key_epoch(&self) -> MasterKeyEpoch {
        self.master_key_epoch
    }

    /// The exact Keyring replica set this commit selects (spec: CP-10, KL-3).
    pub const fn keyring(&self) -> &KeyringCommitment {
        &self.keyring
    }

    /// The slot this record's own successor is committed into, as Storage's
    /// opaque token, and `None` where the provider mints none (spec: CP-2).
    pub fn next_commit_slot(&self) -> Option<&str> {
        self.next_commit_slot.as_deref()
    }

    /// The one slot this head's ordinary Index Snapshot may be created into,
    /// reserved by this record's writer before the commit and in the same form
    /// as the commit slot (spec: CK-10, CP-2).
    ///
    /// `None` where the provider mints no identifier: there the slot is the
    /// Snapshot's name, re-derived from this record's generation at spend time
    /// rather than persisted, so the two spellings cannot drift apart
    /// (spec: CP-15).
    pub fn snapshot_slot(&self) -> Option<&str> {
        self.snapshot_slot.as_deref()
    }

    /// The Containers the batch added, with their entry tables, in Container ID
    /// order (spec: CP-11, FM-15).
    pub fn additions(&self) -> &[ContainerAddition] {
        &self.additions
    }

    /// The Containers the batch removed, in Container ID order.
    ///
    /// A removed Container ID is never added again, so removal from the current
    /// set is monotonic and replaying a record twice removes the same thing
    /// twice (spec: CP-14).
    pub fn removals(&self) -> &[ContainerId] {
        &self.removals
    }

    /// The additions, for a replay that consumes each Container and the Entries
    /// it holds.
    pub fn into_additions(self) -> Vec<ContainerAddition> {
        self.additions
    }
}
