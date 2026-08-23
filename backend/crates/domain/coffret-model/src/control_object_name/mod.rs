use std::fmt;

use crate::container_id::ContainerId;
use crate::control_object_kind::ControlObjectKind;
use crate::error::{Error, Result};
use crate::generation::Generation;
use crate::lowercase_hex::is_nonempty_lowercase_hex;
use crate::replica_position::ReplicaPosition;

mod parse;

#[cfg(test)]
mod tests;

/// The name a control object is stored under.
///
/// Control objects carry recognizable names because recovery discovers them by
/// name before any index exists:
///
/// ```text
/// head-<generation>.cfrt                                 a link in the control-head chain
/// idx-<generation>.cfrt                                  an ordinary Index Snapshot
/// key-<generation>-<set_digest>-r<index>-of-<count>.cfrt  a Keyring replica
/// ```
///
/// A name says what an object is **for**, not what it **is**: the head chain,
/// an ordinary checkpoint, a Keyring replica. Which kind an object is rides in
/// its authenticated header (FM-11), because one head position admits two
/// kinds — the ordinary Journal record and the Index Snapshot that activates a
/// new Master Key epoch both compete for the same successor slot, so naming
/// them differently would leave two keys where the commit protocol needs one
/// (CP-2, CP-3). [`admits`](Self::admits) is the whole of that relation, and
/// parsing a name therefore yields no kind at all.
///
/// A link in the head chain and an Index Snapshot are written once each, so
/// their names carry no replica position and they report
/// [`ReplicaPosition::SINGLE`].
///
/// Numbers are spelled in decimal without leading zeros, so one object has
/// exactly one name: a reader that accepted `head-007.cfrt` as generation 7
/// would let two names claim the same object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ControlObjectName {
    /// A link in the control-head chain: a Journal record, or the Index
    /// Snapshot that activated an epoch at this generation.
    Head {
        /// Which generation of the head chain this object occupies.
        generation: Generation,
    },
    /// An ordinary Index Snapshot.
    IndexSnapshot {
        /// The generation of the head this Snapshot checkpoints (CK-10).
        generation: Generation,
    },
    /// One replica of a Keyring.
    KeyringReplica {
        /// Which generation of the Keyring this replica belongs to.
        generation: Generation,
        /// The digest of the mapping the replica set carries.
        ///
        /// Its contents are the Keyring's business; a name only needs it to be
        /// a lowercase hex token, so that it cannot swallow the separators the
        /// rest of the name is parsed on.
        set_digest: String,
        /// Which replica this is, out of how many.
        replica: ReplicaPosition,
    },
}

/// The name prefix of a link in the control-head chain.
const HEAD_PREFIX: &str = "head-";
/// The name prefix of an ordinary Index Snapshot.
const INDEX_SNAPSHOT_PREFIX: &str = "idx-";
/// The name prefix of a Keyring replica.
const KEYRING_PREFIX: &str = "key-";

impl ControlObjectName {
    /// The name of one generation of the control-head chain.
    pub const fn head(generation: Generation) -> Self {
        Self::Head { generation }
    }

    /// The name the successor of the head at `generation` is created under.
    ///
    /// Both successor kinds derive the same name from the same head, which is
    /// what makes the conditional create that settles a commit a race between
    /// them rather than two uncontested writes (CP-2, CP-3, FM-13).
    pub fn successor_of(generation: Generation) -> Result<Self> {
        Ok(Self::head(generation.next()?))
    }

    /// The name of the ordinary Index Snapshot checkpointing one head.
    pub const fn index_snapshot(generation: Generation) -> Self {
        Self::IndexSnapshot { generation }
    }

    /// The name of one replica of one generation of the Keyring.
    pub fn keyring_replica(
        generation: Generation,
        set_digest: &str,
        replica: ReplicaPosition,
    ) -> Result<Self> {
        // What is refused here is the digest, not a whole name, so the refusal
        // names the digest. `parse` turns it back into
        // `Error::MalformedObjectName` at its own boundary, where the name is
        // what the caller presented.
        if !is_nonempty_lowercase_hex(set_digest) {
            return Err(Error::InvalidSetDigest {
                digest: set_digest.to_owned(),
            });
        }
        Ok(Self::KeyringReplica {
            generation,
            set_digest: set_digest.to_owned(),
            replica,
        })
    }

    /// Whether an object of `kind` may be stored under this name (FM-12).
    ///
    /// Every pairing outside this table is refused before decryption.
    pub const fn admits(&self, kind: ControlObjectKind) -> bool {
        matches!(
            (self, kind),
            (
                Self::Head { .. },
                ControlObjectKind::Journal | ControlObjectKind::ActivationSnapshot
            ) | (Self::IndexSnapshot { .. }, ControlObjectKind::IndexSnapshot)
                | (Self::KeyringReplica { .. }, ControlObjectKind::Keyring)
        )
    }

    /// The generation the name encodes.
    pub const fn generation(&self) -> Generation {
        match self {
            Self::Head { generation }
            | Self::IndexSnapshot { generation }
            | Self::KeyringReplica { generation, .. } => *generation,
        }
    }

    /// The replica position the name encodes.
    pub const fn replica(&self) -> ReplicaPosition {
        match self {
            Self::Head { .. } | Self::IndexSnapshot { .. } => ReplicaPosition::SINGLE,
            Self::KeyringReplica { replica, .. } => *replica,
        }
    }

    /// The digest a Keyring replica's name carries, if this is one.
    pub fn set_digest(&self) -> Option<&str> {
        match self {
            Self::KeyringReplica { set_digest, .. } => Some(set_digest),
            _ => None,
        }
    }
}

impl fmt::Display for ControlObjectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let extension = ContainerId::STORAGE_EXTENSION;
        match self {
            Self::Head { generation } => {
                write!(f, "{HEAD_PREFIX}{generation}{extension}")
            }
            Self::IndexSnapshot { generation } => {
                write!(f, "{INDEX_SNAPSHOT_PREFIX}{generation}{extension}")
            }
            Self::KeyringReplica {
                generation,
                set_digest,
                replica,
            } => write!(
                f,
                "{KEYRING_PREFIX}{generation}-{set_digest}-r{}-of-{}{extension}",
                replica.index(),
                replica.count()
            ),
        }
    }
}
