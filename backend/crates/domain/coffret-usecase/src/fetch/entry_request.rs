use coffret_model::EntryPath;

use crate::commit::CommitPolicy;
use crate::device_state::DeviceTime;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;

/// Everything one run of [`fetch_entry`](super::fetch_entry) works from.
///
/// The same ports, keys, clock, and policy [`FetchRequest`](super::FetchRequest)
/// takes, and one Entry Path instead of a prefix. Where the file goes is still
/// not among them: that is the device's mappings, which the [`Index`] holds
/// (spec: EP-9), so a caller cannot fetch an Entry into a folder the Library does
/// not know this device has.
pub struct FetchEntryRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// The Entry to make available on this device.
    pub path: EntryPath,
    /// What this device's clock says as the run starts.
    ///
    /// The observation the run writes down is stamped with it. Nothing about the
    /// Library's correctness rests on it (spec: CP-7).
    pub now: DeviceTime,
    /// The decisions Storage does not make.
    ///
    /// A partial fetch commits nothing, so what it takes from the policy is
    /// the [`RetryPolicy`](crate::RetryPolicy): the catch-up it starts with, the
    /// committed Keyring it opens, and every ranged read it makes run under it.
    pub policy: CommitPolicy,
}

impl<'a> FetchEntryRequest<'a> {
    /// A run against `store` and `index` for the Entry at one path, under the
    /// default policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        path: EntryPath,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            path,
            now,
            policy: CommitPolicy::default(),
        }
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }
}
