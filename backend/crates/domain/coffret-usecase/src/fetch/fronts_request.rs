use crate::commit::CommitPolicy;
use crate::fetch::fetch_memory::FetchMemory;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;

/// Everything one [`read_fronts`](super::read_fronts) works from.
///
/// The ports, the keys and the memory an Entry fetch answering from the catalog
/// as it stands takes, because these reads are that fetch's first step taken
/// early: the same Storage, the same listing, the same committed key table, and
/// the same place the fronts are kept in for it.
pub struct FrontsRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// Where the fronts are kept, and where the listing and key table they are
    /// read through are kept already.
    pub memory: &'a FetchMemory,
    /// The decisions Storage does not make; the retry policy is what these
    /// reads take of it.
    pub policy: CommitPolicy,
}

impl<'a> FrontsRequest<'a> {
    /// Reads against `store` and `index`, kept in `memory`, under the default
    /// policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        memory: &'a FetchMemory,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            memory,
            policy: CommitPolicy::default(),
        }
    }
}
