//! The payload of an Index Snapshot, ordinary and epoch-activating (FM-16).
//!
//! A Snapshot is the Index of the whole Library at one committed state: the
//! checkpoint it stands at (CK-1, CK-2, CK-3), every current Container, and
//! every current Entry with the Container that holds it. Both Snapshot kinds
//! carry that same content, and the activation kind carries beyond it the two
//! fields that say which head it fenced (MR-2) — so one schema serves both, and
//! which of them an object is stays where FM-11 put it: in the authenticated
//! header.
//!
//! An Entry names its Container by index into `containers` rather than by ID,
//! because a Library holds far more Entries than Containers and the 16-byte ID
//! would otherwise be repeated once per Entry. That index is the one thing a
//! reader has to check beyond the field shapes: an index past the end of
//! `containers` is a Snapshot that cannot be read back into an Index at all.
//!
//! What a Snapshot never carries is device state (CK-7) — including
//! [`SnapshotContent::adopted_from`](coffret_model::SnapshotContent::adopted_from),
//! which is this Index's own provenance rather than Library content. The
//! encoder ignores it and the decoder yields `None`.

use coffret_model::{ControlObjectKind, SnapshotContent};

mod encode;
pub use encode::encode;

mod decode;
pub use decode::decode;

mod snapshot_activation;
pub use snapshot_activation::SnapshotActivation;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;
#[cfg(test)]
mod size_tests;

#[cfg(test)]
mod testing;

/// The schema this crate writes for an Index Snapshot payload (FM-16).
const SCHEMA: u64 = 1;

const HEAD_GENERATION: &str = "head_generation";
const JOURNAL_GENERATION: &str = "journal_generation";
const NEXT_COMMIT_SLOT: &str = "next_commit_slot";
const KEYRING_GENERATION: &str = "keyring_generation";
const KEYRING_REPLICA_COUNT: &str = "keyring_replica_count";
const KEYRING_SET_DIGEST: &str = "keyring_set_digest";
const CONTAINERS: &str = "containers";
const ENTRIES: &str = "entries";
const CONTAINER: &str = "container";
const BASE_HEAD_GENERATION: &str = "base_head_generation";
const ACTIVATION_SLOT: &str = "activation_slot";

/// One Index Snapshot payload: the Library-wide content, activation or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSnapshotPayload {
    /// The Library-wide content this Snapshot holds (spec: CK-7).
    pub content: SnapshotContent,
    /// Set on an activation Snapshot, absent on an ordinary one (spec: MR-2).
    pub activation: Option<SnapshotActivation>,
}

impl IndexSnapshotPayload {
    /// The ordinary checkpoint of one head (spec: CK-10).
    pub fn ordinary(content: SnapshotContent) -> Self {
        Self {
            content,
            activation: None,
        }
    }

    /// The Snapshot that activates an epoch by taking a head position
    /// (spec: MR-2).
    pub fn activating(content: SnapshotContent, activation: SnapshotActivation) -> Self {
        Self {
            content,
            activation: Some(activation),
        }
    }

    /// Which control-object kind this payload has to be framed as (FM-11).
    ///
    /// The two kinds share this schema, so the kind follows from whether the
    /// activation fields are here rather than from a flag a caller could set
    /// against them.
    pub fn control_object_kind(&self) -> ControlObjectKind {
        match self.activation {
            Some(_) => ControlObjectKind::ActivationSnapshot,
            None => ControlObjectKind::IndexSnapshot,
        }
    }
}
