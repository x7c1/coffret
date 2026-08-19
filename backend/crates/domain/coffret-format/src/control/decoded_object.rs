use coffret_model::{ControlObjectKind, Generation, ReplicaPosition};

use super::payload::ControlPayload;

/// An opened control object: what its header said, and what it carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedControlObject {
    /// Which kind of control state this object carries.
    pub kind: ControlObjectKind,
    /// How many times this kind had been rewritten when it was written.
    pub generation: Generation,
    /// Which replica this is, out of how many.
    pub replica: ReplicaPosition,
    /// The payload, epoch first.
    pub payload: ControlPayload,
}
