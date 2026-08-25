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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexCheckpoint {
    /// Which Master Key encrypted the control state this checkpoint stands on
    /// (spec: CK-3, CP-13, FM-13).
    pub master_key_epoch: MasterKeyEpoch,
    /// The control-head generation this checkpoint represents (spec: CK-1).
    pub head_generation: Generation,
    /// The last Journal generation applied to reach it (spec: CK-1, CK-4).
    pub journal_generation: Generation,
    /// The slot this head's successor is committed into, as Storage's own
    /// opaque token (spec: CK-2, CP-2).
    ///
    /// It is `None` where the provider keys objects by name and so mints
    /// nothing: there the slot is the successor's name, which is re-derived
    /// from the head generation and the successor's role at spend time rather
    /// than persisted, so the two spellings cannot drift apart (spec: CP-2,
    /// CP-15).
    pub next_commit_slot: Option<String>,
    /// The exact Keyring replica set the commit behind this head selected
    /// (spec: CK-3, CP-10, KL-3).
    pub keyring: KeyringCommitment,
}
