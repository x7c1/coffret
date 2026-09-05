use anyhow::Result;
use coffret_model::{Generation, MasterKeyEpoch, ReplicaPosition};
use serde::{Deserialize, Serialize};

use super::{BodyField, WireControlObjectKind};

/// One control object in a fixture set, with what it must decode to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlObjectFixture {
    /// The name this fixture is known by across both implementations.
    pub fixture: String,
    /// Where the bytes live, relative to the fixture directory.
    pub file: String,
    /// The name the object is stored under, in one of FM-12's forms.
    pub object_name: String,
    /// Which kind of control state the object carries.
    pub kind: WireControlObjectKind,
    /// Where the object sits in the Library's control history; the numbering
    /// never restarts at a rotation (FM-13).
    pub generation: u64,
    /// Which replica this is.
    pub replica_index: u16,
    /// How many replicas the set declares.
    pub replica_count: u16,
    /// The Master Key epoch that encrypted the payload.
    pub master_key_epoch: u64,
    /// The kind's own payload fields.
    pub body: Vec<BodyField>,
}

impl ControlObjectFixture {
    /// The generation this fixture states.
    pub fn generation(&self) -> Result<Generation> {
        Ok(Generation::new(self.generation)?)
    }

    /// The replica position this fixture states.
    pub fn replica(&self) -> Result<ReplicaPosition> {
        Ok(ReplicaPosition::new(
            self.replica_index,
            self.replica_count,
        )?)
    }

    /// The Master Key epoch this fixture states.
    pub fn master_key_epoch(&self) -> Result<MasterKeyEpoch> {
        Ok(MasterKeyEpoch::new(self.master_key_epoch)?)
    }
}
