use crate::commit::commit_policy::CommitPolicy;
use crate::commit::control_keys::ControlKeys;
use crate::commit::keyring::DegradedReport;
use crate::commit::prepared_batch::PreparedBatch;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// Everything one run of [`commit_batch`](super::commit_batch) works from.
///
/// The two ports, the keys of the epoch the Library is in, the policy decisions
/// Storage does not make, and the batch itself. Nothing else about the commit
/// is passed in: what generation it takes, which head it succeeds, and which
/// Keyring it selects are read off the Library, because a caller that could
/// state them could state them wrongly.
///
/// One thing travels the other way, within this crate, and it decides nothing
/// the commit does: a caller that read the committed Keyring before it built
/// this batch may hand the finding that read left along with it, so that the
/// examination inside this run speaks for it rather than the caller saying the
/// same thing over again.
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
    /// The caller's held finding about the committed Keyring, where it read the
    /// set before coming here.
    ///
    /// Nothing the commit decides turns on it: the examination reads the set
    /// itself. It is here only so that the run's one word about a set found
    /// short comes from the walk that also repairs it (spec: KL-15).
    pub(crate) degraded: Option<&'a DegradedReport>,
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
            degraded: None,
        }
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// The same request, carrying the finding the caller's own read of the
    /// committed Keyring left.
    ///
    /// A run that gets as far as examining that set says what it found and
    /// what it put back, so the finding is spoken for there and the caller's
    /// guard stays silent (spec: KL-11, KL-15).
    pub(crate) fn speaking_for(mut self, degraded: Option<&'a DegradedReport>) -> Self {
        self.degraded = degraded;
        self
    }
}
