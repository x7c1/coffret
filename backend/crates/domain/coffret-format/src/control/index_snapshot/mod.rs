//! The payload of an Index Snapshot, ordinary and epoch-activating
//! (spec: FM-16).
//!
//! A Snapshot is the Index of the whole Library at one committed state: the
//! checkpoint it stands at (spec: CK-1, CK-2, CK-3), every current Container,
//! and every current Entry with the Container that holds it. Both Snapshot
//! kinds carry that same content, and the activation kind carries beyond it the
//! two fields that say which head it fenced (spec: MR-2) — so one schema serves
//! both, and which of them an object is stays where FM-11 put it: in the
//! authenticated header.
//!
//! An Entry names its Container by index into `containers` rather than by ID,
//! because a Library holds far more Entries than Containers and the 16-byte ID
//! would otherwise be repeated once per Entry. That index is the one thing a
//! reader has to check beyond the field shapes: an index past the end of
//! `containers` is a Snapshot that cannot be read back into an Index at all.
//!
//! Beside it each Entry carries `entry`, its entry number in that Container's
//! table (spec: FM-9). The Snapshot's own order is the Entry Path order, so the
//! position would otherwise be lost. And each carries `deposited_at`, when the
//! content now at its path was deposited into the Library, which a catalog
//! rebuilt from the Snapshot holds exactly as the records left it
//! (spec: FM-16).
//!
//! What a Snapshot never carries is device state (spec: CK-7) — including
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

/// The schema this crate writes for an Index Snapshot payload (spec: FM-16).
///
/// Not what a reader refuses below: that is [`LOWEST_READ_SCHEMA`], so that a
/// later build writing a new field under a higher schema goes on reading the
/// Snapshots written before it.
const SCHEMA: u64 = 4;

/// The lowest schema a reader accepts: 4, the first whose Entries carry
/// `deposited_at` (spec: FM-16).
///
/// A Snapshot of an earlier schema lacks the deposit time of every Entry it
/// lists, and the records it checkpoints lack it too, so nothing could supply
/// one: it is refused as a schema this build does not read, and a Library
/// holding one is recreated rather than migrated.
///
/// Equal to [`SCHEMA`] for now.
const LOWEST_READ_SCHEMA: u64 = 4;

const HEAD_GENERATION: &str = "head_generation";
const JOURNAL_GENERATION: &str = "journal_generation";
const NEXT_COMMIT_SLOT: &str = "next_commit_slot";
const KEYRING_GENERATION: &str = "keyring_generation";
const KEYRING_REPLICA_COUNT: &str = "keyring_replica_count";
const KEYRING_SET_DIGEST: &str = "keyring_set_digest";
const CONTAINERS: &str = "containers";
const ENTRIES: &str = "entries";
const CONTAINER: &str = "container";
const ENTRY: &str = "entry";
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

    /// Which control-object kind this payload has to be framed as
    /// (spec: FM-11).
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
