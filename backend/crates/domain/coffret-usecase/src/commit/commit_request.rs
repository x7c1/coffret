use crate::commit::commit_policy::CommitPolicy;
use crate::commit::control_keys::ControlKeys;
use crate::commit::prepared_batch::PreparedBatch;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// Everything one run of [`commit_batch`](super::commit_batch) works from.
///
/// The two ports, the keys of the epoch the Library is in, the policy decisions
/// Storage does not make, and the batch itself. Nothing else: what generation
/// the commit takes, which head it succeeds, and which Keyring it selects are
/// read off the Library rather than passed in, because a caller that could
/// state them could state them wrongly.
pub struct CommitRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The control-object keys of the epoch the Library is in.
    pub keys: &'a ControlKeys,
    /// The decisions Storage does not make.
    pub policy: CommitPolicy,
    /// The batch to commit.
    pub batch: PreparedBatch,
}

impl<'a> CommitRequest<'a> {
    /// A request to commit `batch` against `store` and `index`, with the
    /// default policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a ControlKeys,
        batch: PreparedBatch,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            policy: CommitPolicy::default(),
            batch,
        }
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }
}
