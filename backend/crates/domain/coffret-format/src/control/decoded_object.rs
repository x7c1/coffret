use coffret_model::{ControlObjectKind, Generation, ReplicaPosition};

use super::payload::ControlPayload;

/// An opened control object: what its header said, and what it carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedControlObject {
    /// Which kind of control state this object carries.
    pub kind: ControlObjectKind,
    /// Where this object sits in the Library's control history (FM-13).
    ///
    /// For a Journal record or an activation Index Snapshot that is its place
    /// in the head chain the two kinds share; for an ordinary Index Snapshot,
    /// the generation of the head it checkpoints; for a Keyring, the Keyring's
    /// own counter. None of them restarts at a Master Key rotation.
    pub generation: Generation,
    /// Which replica this is, out of how many.
    pub replica: ReplicaPosition,
    /// The payload, epoch first.
    pub payload: ControlPayload,
}
