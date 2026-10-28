use std::error;
use std::fmt;

use coffret_model::{ControlObjectName, KeyringCommitment, MasterKeyEpoch, Redacted};

use crate::commit::ShortKeyring;

/// Why a prune deleted nothing (spec: CK-5).
///
/// Each is a state of the Library a person can see, and none of them removed
/// anything. The Snapshot a refusal names stays on Storage: it was written, or
/// found, before the gate was asked (spec: CK-8), and it is a valid checkpoint
/// whatever the gate decided — a catch-up starts from it as from any other.
///
/// Nothing here is an Entry Path or a local path, so the message and the
/// redacted form say the same thing (spec: EL-1).
#[derive(Debug, Clone)]
pub enum PruneRefusal {
    /// The Library has committed nothing — not even its first head — so there
    /// is no head to checkpoint and no record to remove (spec: FM-13).
    NothingCommitted,
    /// The Snapshot does not record the Keyring tuple the head committed
    /// (spec: CK-3, CK-5, KL-3).
    ///
    /// A Snapshot of the head is meant to carry exactly what the head selected,
    /// so this is a Snapshot another writer got wrong; once the records were
    /// gone it would be the only proof of which replica set the Library
    /// selected, and it proves the wrong one.
    KeyringNotCommitted {
        /// The Snapshot that records the other tuple.
        snapshot: ControlObjectName,
        /// The epoch the Snapshot records.
        recorded_epoch: MasterKeyEpoch,
        /// The Keyring commitment the Snapshot records.
        recorded: KeyringCommitment,
        /// The epoch the head committed under.
        committed_epoch: MasterKeyEpoch,
        /// The Keyring commitment the head committed.
        committed: KeyringCommitment,
    },
    /// The committed Keyring set is not complete on Storage: a position its
    /// count declares is lost, or Storage would not hand it over (spec: KL-2,
    /// CK-5).
    ///
    /// Once the records were gone the replicas would be the envelopes' only
    /// carriers, so the set has to be whole first. The next commit's
    /// examination puts a lost position back (spec: KL-13); a prune after it
    /// goes ahead.
    KeyringIncomplete {
        /// The Snapshot the run rested on.
        snapshot: ControlObjectName,
        /// What the read of every position found.
        short: ShortKeyring,
    },
}

impl fmt::Display for PruneRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingCommitted => {
                f.write_str("the Library has committed nothing, so there is nothing to prune")
            }
            Self::KeyringNotCommitted {
                snapshot,
                recorded_epoch,
                recorded,
                committed_epoch,
                committed,
            } => write!(
                f,
                "the checkpoint {snapshot} records Keyring generation {} ({} replicas) of \
                 epoch {recorded_epoch}, but the head committed generation {} ({} replicas) \
                 of epoch {committed_epoch}",
                recorded.generation(),
                recorded.replica_count(),
                committed.generation(),
                committed.replica_count(),
            ),
            Self::KeyringIncomplete { short, .. } if short.established() => write!(
                f,
                "the committed Keyring generation {} is missing {} of its {} replicas; the \
                 next sync, freeze or rename that commits a change repairs it",
                short.generation(),
                short.lost(),
                short.replicas(),
            ),
            Self::KeyringIncomplete { short, .. } => write!(
                f,
                "Storage did not hand over {} of the {} replicas of the committed Keyring \
                 generation {}, so the set is not known to be complete",
                short.unfetched(),
                short.replicas(),
                short.generation(),
            ),
        }
    }
}

impl error::Error for PruneRefusal {}

impl Redacted for PruneRefusal {
    /// Generations, counts and an object name, which is all any variant holds.
    fn redacted(&self) -> String {
        match self {
            Self::NothingCommitted => "PruneRefusal::NothingCommitted".to_owned(),
            Self::KeyringNotCommitted {
                snapshot,
                recorded,
                committed,
                ..
            } => format!(
                "PruneRefusal::KeyringNotCommitted(snapshot={snapshot}, recorded={}, \
                 committed={})",
                recorded.generation(),
                committed.generation(),
            ),
            Self::KeyringIncomplete { snapshot, short } => format!(
                "PruneRefusal::KeyringIncomplete(snapshot={snapshot}, generation={}, \
                 replicas={}, lost={}, unfetched={})",
                short.generation(),
                short.replicas(),
                short.lost(),
                short.unfetched(),
            ),
        }
    }
}
