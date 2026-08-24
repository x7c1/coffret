use coffret_model::ContainerId;

use crate::commit::prepared_addition::PreparedAddition;
use crate::device_state::LocalObservation;

/// A batch whose Containers are on Storage and whose commit has not happened.
///
/// Everything here was decided before the flow starts: which Containers the
/// batch adds, which it removes, and which local files this device put in place
/// while producing it. The flow adds only what the Library's current state
/// determines — the generation, the head it succeeds, the Keyring the commit
/// selects — so the same prepared batch survives a rebase onto a new head
/// unchanged (spec: CP-4, EP-7).
///
/// A batch reaching this shape says nothing about the Library yet: until its
/// Journal record exists it has changed nothing (spec: CP-1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedBatch {
    /// The Containers the batch adds, with the keys that open them.
    pub additions: Vec<PreparedAddition>,
    /// The Containers the batch removes from the current set.
    pub removals: Vec<ContainerId>,
    /// The local files this device materialized while producing the batch.
    ///
    /// Device state rather than Library content, so it reaches the Index
    /// through [`Index::refresh`](crate::Index::refresh) and no Storage Object
    /// ever carries it (spec: EP-10, CK-7).
    pub materialized: Vec<LocalObservation>,
}

impl PreparedBatch {
    /// A batch that adds `additions` and removes nothing.
    pub fn adding(additions: Vec<PreparedAddition>) -> Self {
        Self {
            additions,
            removals: Vec::new(),
            materialized: Vec::new(),
        }
    }

    /// The same batch, removing `removals` as well.
    pub fn removing(mut self, removals: Vec<ContainerId>) -> Self {
        self.removals = removals;
        self
    }

    /// The same batch, recording what this device put on disk for it.
    pub fn materializing(mut self, materialized: Vec<LocalObservation>) -> Self {
        self.materialized = materialized;
        self
    }
}
