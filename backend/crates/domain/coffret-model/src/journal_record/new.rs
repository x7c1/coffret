use super::JournalRecord;
use crate::canonical_order::{require_strictly_increasing, ADDITIONS, REMOVALS};
use crate::container_addition::ContainerAddition;
use crate::container_id::ContainerId;
use crate::error::{Error, Result};
use crate::generation::Generation;
use crate::keyring_commitment::KeyringCommitment;
use crate::master_key_epoch::MasterKeyEpoch;

impl JournalRecord {
    /// The record a commit writes, or a refusal where it is not one a commit
    /// could have written.
    ///
    /// A record at generation *g* succeeds head *g − 1*, and the record at
    /// generation 0 succeeds nothing: those are the only two shapes `prev` may
    /// take, so a record whose own two statements disagree would make a replay
    /// follow a chain no commit ever wrote (spec: FM-15).
    ///
    /// # Errors
    ///
    /// - [`Error::JournalRecordPredecessorMismatch`] where `prev` is not the
    ///   head this generation succeeds (spec: FM-15).
    /// - [`Error::CollectionOutOfCanonicalOrder`] where `additions` or
    ///   `removals` is not strictly increasing by Container ID (spec: FM-15).
    ///
    /// Whether an ID may appear in both collections is not a rule FM-15 states,
    /// and none is invented here.
    // The record's fields are the record's fields: eight of them are what
    // FM-15 defines, and grouping some of them behind a type of their own
    // would be inventing a structure the format does not have.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        generation: Generation,
        prev: Option<Generation>,
        master_key_epoch: MasterKeyEpoch,
        keyring: KeyringCommitment,
        next_commit_slot: Option<String>,
        snapshot_slot: Option<String>,
        additions: Vec<ContainerAddition>,
        removals: Vec<ContainerId>,
    ) -> Result<Self> {
        if prev != predecessor_of(generation) {
            return Err(Error::JournalRecordPredecessorMismatch { generation, prev });
        }
        require_strictly_increasing(ADDITIONS, &additions, |left, right| {
            left.container().id.cmp(&right.container().id)
        })?;
        require_strictly_increasing(REMOVALS, &removals, Ord::cmp)?;
        Ok(Self {
            generation,
            prev,
            master_key_epoch,
            keyring,
            next_commit_slot,
            snapshot_slot,
            additions,
            removals,
        })
    }

    /// The same record from collections in whatever order a writer gathered
    /// them: sorted by Container ID, then held to [`new`](Self::new)'s rules.
    ///
    /// A writer collects its additions in the order it spooled them and its
    /// removals in the order the Containers they displace turned up, and the
    /// order FM-15 fixes is what makes one batch one encoding — so the sort
    /// belongs on the way in, once, rather than at the encoder.
    ///
    /// Sorting cannot make a Container named twice disappear, so what this
    /// refuses is exactly what `new` refuses once the order is no longer in
    /// question.
    ///
    /// # Errors
    ///
    /// [`new`](Self::new)'s, on its terms.
    #[allow(clippy::too_many_arguments)]
    pub fn canonical(
        generation: Generation,
        prev: Option<Generation>,
        master_key_epoch: MasterKeyEpoch,
        keyring: KeyringCommitment,
        next_commit_slot: Option<String>,
        snapshot_slot: Option<String>,
        mut additions: Vec<ContainerAddition>,
        mut removals: Vec<ContainerId>,
    ) -> Result<Self> {
        additions.sort_by_key(|addition| addition.container().id);
        removals.sort_unstable();
        Self::new(
            generation,
            prev,
            master_key_epoch,
            keyring,
            next_commit_slot,
            snapshot_slot,
            additions,
            removals,
        )
    }
}

/// The head a record at `generation` succeeds, and `None` at generation 0.
///
/// The Library's first head was built on nothing, so it is the one record that
/// states no predecessor (spec: FM-13).
fn predecessor_of(generation: Generation) -> Option<Generation> {
    generation.get().checked_sub(1).map(Generation::new)
}
