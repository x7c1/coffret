use coffret_model::{ContainerId, Generation};

use crate::commit::pending_rename::PendingRename;
use crate::commit::prepared_addition::PreparedAddition;
use crate::device_state::LocalObservation;

/// A batch whose Containers are on Storage and whose commit has not happened.
///
/// Everything here was decided before the flow starts: which Containers the
/// batch adds, which it removes, which Entries it renames, and which local
/// files this device put in place while producing it. The flow adds only what
/// the Library's current state determines — the generation, the head it
/// succeeds, the Keyring the commit selects — and what the records committed
/// since the batch was prepared did to the Entries it names: a rebase applies
/// their renames to the pending changes, by `(Container ID, entry number)`,
/// and refuses the batch where the two cannot both stand (spec: CP-4, EP-7).
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
    /// The Entries the batch gives a new current Entry Path (spec: FM-15).
    pub renames: Vec<PendingRename>,
    /// The head the batch was prepared against: what the catalog its producer
    /// read stood at.
    ///
    /// Every record after it is one the batch did not see, and is what a
    /// rebase examines (spec: CP-4, EP-7). `None` takes the head the Index
    /// stands at when the commit is asked for, which is the head a producer
    /// that read the catalog just before handing the batch over read too.
    pub(crate) base: Option<Generation>,
}

impl PreparedBatch {
    /// A batch that adds `additions` and removes nothing.
    pub fn adding(additions: Vec<PreparedAddition>) -> Self {
        Self {
            additions,
            ..Self::default()
        }
    }

    /// The same batch, removing `removals` as well.
    pub fn removing(mut self, removals: Vec<ContainerId>) -> Self {
        self.removals = removals;
        self
    }

    /// The same batch, renaming `renames` as well.
    ///
    /// No operation writes renames yet; this is how one hands them over.
    pub fn renaming(mut self, renames: Vec<PendingRename>) -> Self {
        self.renames = renames;
        self
    }

    /// The same batch, prepared against the head at `base`.
    pub(crate) fn prepared_at(mut self, base: Option<Generation>) -> Self {
        self.base = base;
        self
    }

    /// Whether the batch changes nothing: it adds no Container, removes none,
    /// and renames no Entry.
    ///
    /// What a commit writes for such a batch is a record with nothing in it,
    /// which is exactly a Library's first head (spec: CP-2, FM-13) and nothing
    /// a later head needs.
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty() && self.removals.is_empty() && self.renames.is_empty()
    }

    /// The same batch, recording what this device put on disk for it.
    pub fn materializing(mut self, materialized: Vec<LocalObservation>) -> Self {
        self.materialized = materialized;
        self
    }
}
