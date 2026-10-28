use crate::commit::CommitPolicy;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;

/// Everything one run of [`prune_journal`](super::prune_journal) works from.
///
/// The two ports, the epoch's keys, and how long a Storage call is worth
/// retrying. No spool and no clock: a prune commits nothing and places
/// nothing, so it has no batch of this device's to complete and no file to
/// stamp.
pub struct PruneRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library, which the Snapshot is written
    /// from where the head has none yet (spec: CK-7, CK-8).
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    ///
    /// What the run spends is [`LibraryKeys::control`]: it opens control
    /// objects and seals a Snapshot, and never opens a Container.
    pub keys: &'a LibraryKeys,
    /// The decisions Storage does not make.
    ///
    /// What a prune takes from it is the [`RetryPolicy`](crate::RetryPolicy);
    /// it takes the whole policy because its catch-up and its checkpoint are
    /// the commit flow's own, and speak in the commit flow's terms.
    pub policy: CommitPolicy,
}

impl<'a> PruneRequest<'a> {
    /// A run against `store` and `index` under the default policy.
    pub fn new(store: &'a dyn ObjectStore, index: &'a dyn Index, keys: &'a LibraryKeys) -> Self {
        Self {
            store,
            index,
            keys,
            policy: CommitPolicy::default(),
        }
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }
}
