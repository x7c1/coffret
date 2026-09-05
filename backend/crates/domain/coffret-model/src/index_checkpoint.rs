use crate::error::{Error, Result};
use crate::generation::Generation;
use crate::keyring_commitment::KeyringCommitment;
use crate::master_key_epoch::MasterKeyEpoch;

/// The committed Library state an Index stands at.
///
/// It is exactly what an Index Snapshot records as its checkpoint, so a device
/// that adopts a Snapshot adopts this and a device that writes one writes this
/// (spec: CK-1, CK-2, CK-3, CP-6).
///
/// The two generations are not the same number and are both needed. Recovery
/// starts from the head generation and replays the Journal successors after it,
/// while the last applied Journal generation is what says which records have
/// become eligible for `prune` (spec: CK-1, CK-4). They coincide after an
/// ordinary commit and diverge after an epoch activation, whose Snapshot
/// occupies a head position without being a Journal record (spec: CP-6, FM-12).
///
/// Which way they may diverge is the one rule this type holds, and it holds it
/// for every reader at once: a checkpoint out of a payload, out of a catalog
/// row, or out of a replay is built through [`new`](Self::new) or is not built
/// at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexCheckpoint {
    master_key_epoch: MasterKeyEpoch,
    head_generation: Generation,
    journal_generation: Generation,
    next_commit_slot: Option<String>,
    keyring: KeyringCommitment,
}

impl IndexCheckpoint {
    /// The state an Index stands at, or a refusal where the two generations
    /// stand in an order no commit produces.
    ///
    /// # Errors
    ///
    /// [`Error::CheckpointJournalAheadOfHead`] where `journal_generation` is
    /// past `head_generation` (spec: CK-1).
    pub fn new(
        master_key_epoch: MasterKeyEpoch,
        head_generation: Generation,
        journal_generation: Generation,
        next_commit_slot: Option<String>,
        keyring: KeyringCommitment,
    ) -> Result<Self> {
        if journal_generation > head_generation {
            return Err(Error::CheckpointJournalAheadOfHead {
                head_generation,
                journal_generation,
            });
        }
        Ok(Self {
            master_key_epoch,
            head_generation,
            journal_generation,
            next_commit_slot,
            keyring,
        })
    }

    /// The state reached by applying the Journal up to the head itself.
    ///
    /// The one construction with nothing to refuse, which is why it is the one
    /// that answers without a `Result`: a Journal record is both a head and the
    /// last Journal generation applied to reach it, so the two generations
    /// coincide and CK-1's rule holds by construction (spec: CK-1, CP-6).
    pub const fn at_head(
        master_key_epoch: MasterKeyEpoch,
        head_generation: Generation,
        next_commit_slot: Option<String>,
        keyring: KeyringCommitment,
    ) -> Self {
        Self {
            master_key_epoch,
            head_generation,
            journal_generation: head_generation,
            next_commit_slot,
            keyring,
        }
    }

    /// Which Master Key encrypted the control state this checkpoint stands on
    /// (spec: CK-3, CP-13, FM-13).
    pub const fn master_key_epoch(&self) -> MasterKeyEpoch {
        self.master_key_epoch
    }

    /// The control-head generation this checkpoint represents (spec: CK-1).
    pub const fn head_generation(&self) -> Generation {
        self.head_generation
    }

    /// The last Journal generation applied to reach it (spec: CK-1, CK-4).
    pub const fn journal_generation(&self) -> Generation {
        self.journal_generation
    }

    /// The slot this head's successor is committed into, as Storage's own
    /// opaque token (spec: CK-2, CP-2).
    ///
    /// It is `None` where the provider keys objects by name and so mints
    /// nothing: there the slot is the successor's name, which is re-derived
    /// from the head generation and the successor's role at spend time rather
    /// than persisted, so the two spellings cannot drift apart (spec: CP-2,
    /// CP-15).
    pub fn next_commit_slot(&self) -> Option<&str> {
        self.next_commit_slot.as_deref()
    }

    /// The exact Keyring replica set the commit behind this head selected
    /// (spec: CK-3, CP-10, KL-3).
    pub const fn keyring(&self) -> &KeyringCommitment {
        &self.keyring
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{generation, keyring_commitment, master_key_epoch};

    /// The checkpoint standing at head `head` having applied Journal `journal`.
    fn checkpoint(head: u64, journal: u64) -> Result<IndexCheckpoint> {
        IndexCheckpoint::new(
            master_key_epoch(),
            generation(head),
            generation(journal),
            None,
            keyring_commitment(),
        )
    }

    // CK-1: the two generations coincide after an ordinary commit and diverge
    // only downwards, at an activation whose Snapshot takes a head position
    // without being a Journal record. One past the head names records applied
    // to reach a state the head does not cover.
    #[test]
    fn a_checkpoint_whose_journal_is_ahead_of_its_head_cannot_exist() {
        let result = checkpoint(4, 5);

        assert!(
            matches!(
                result,
                Err(Error::CheckpointJournalAheadOfHead {
                    head_generation,
                    journal_generation,
                }) if head_generation.get() == 4 && journal_generation.get() == 5
            ),
            "expected the pair to be refused with both generations, got {result:?}",
        );

        let equal = checkpoint(4, 4).expect("an ordinary commit leaves the two equal");
        assert_eq!(equal.head_generation(), equal.journal_generation());
        checkpoint(4, 1).expect("an activation leaves the Journal generation behind the head");
    }
}
