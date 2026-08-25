use coffret_model::EntryPath;

use crate::commit::CommitPolicy;
use crate::device_state::DeviceTime;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;

/// Everything one run of [`fetch_folders`](super::fetch_folders) works from.
///
/// The two ports, the epoch's keys, the one value a device supplies rather than
/// derives — what its clock says — and the decisions Storage does not make.
/// Where the files go is not among them: that is the device's mappings, which
/// the [`Index`] holds (spec: EP-9), so a caller cannot fetch the Library into a
/// folder the Library does not know it has.
///
/// There is no spool directory. A fetch writes its temporary file into the
/// destination directory itself, because the rename that makes a verified file
/// visible has to be a rename within one filesystem (spec: EP-11).
pub struct FetchRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// The subtree to fetch, or `None` for everything the mappings cover.
    ///
    /// It narrows the run and never widens it: a prefix outside every mapping
    /// selects nothing, because a mapping is what says where a file could go at
    /// all (spec: EP-9). Where a prefix and a mapping overlap, the run covers
    /// the deeper of the two.
    pub prefix: Option<EntryPath>,
    /// What this device's clock says as the run starts.
    ///
    /// Every observation the run writes down is stamped with it, so one run's
    /// bookkeeping stands at one moment rather than at as many moments as it
    /// placed files. Nothing about the Library's correctness rests on it
    /// (spec: CP-7).
    pub now: DeviceTime,
    /// The decisions Storage does not make.
    ///
    /// A fetch commits nothing, so what it takes from the policy is the
    /// [`RetryPolicy`](crate::RetryPolicy): the catch-up it starts with and every
    /// object it pulls back run under it. It takes the whole policy rather than
    /// that one field because the catch-up is the commit flow's routine and
    /// speaks in the commit flow's terms.
    pub policy: CommitPolicy,
}

impl<'a> FetchRequest<'a> {
    /// A run against `store` and `index` covering everything the mappings cover,
    /// under the default policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            prefix: None,
            now,
            policy: CommitPolicy::default(),
        }
    }

    /// The same request narrowed to one subtree of the Library.
    pub fn under(mut self, prefix: EntryPath) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }
}
