use crate::commit::CommitPolicy;
use crate::fetch::FetchMemory;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;

/// Everything one run of [`catch_up_catalog`](super::catch_up_catalog) works
/// from.
///
/// The two ports, the epoch's keys, and how long a Storage call is worth
/// retrying. There is no prefix, no clock, and no spool: a catch-up covers the
/// Library entire because a Journal record does — it is replayed whole or not at
/// all (spec: CK-9) — it writes nothing this device would have to stamp with a
/// time, and it uploads nothing.
pub struct CatchUpRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    ///
    /// What the run actually spends is [`LibraryKeys::control`], the four
    /// control-object keys: nothing here opens a Container, so the key that
    /// unwraps a Container Key is never reached for. It takes the whole bundle
    /// anyway, because that is what a device holds and splitting it here would
    /// let a caller hand over the control half of one epoch.
    pub keys: &'a LibraryKeys,
    /// The decisions Storage does not make.
    ///
    /// A catch-up commits nothing, so what it takes from the policy is the
    /// [`RetryPolicy`](crate::RetryPolicy). It takes the whole policy rather than
    /// that one field because the catch-up is the commit flow's own routine and
    /// speaks in the commit flow's terms, exactly as a fetch's request does.
    pub policy: CommitPolicy,
    /// What a process keeps between its Entry fetches, where it keeps anything.
    ///
    /// A catch-up walks the Library's listing to reach its head, and that walk
    /// is the very listing those fetches reach objects through; a process whose
    /// fetches answer from the catalog as it stands is handed it here, rather
    /// than having each of them list again. `None`, the default, keeps nothing.
    pub memory: Option<&'a FetchMemory>,
}

impl<'a> CatchUpRequest<'a> {
    /// A run against `store` and `index` under the default policy.
    pub fn new(store: &'a dyn ObjectStore, index: &'a dyn Index, keys: &'a LibraryKeys) -> Self {
        Self {
            store,
            index,
            keys,
            policy: CommitPolicy::default(),
            memory: None,
        }
    }

    /// The same run, leaving the listing it walked in `memory`, with the head
    /// it brought the catalog to (see [`FetchMemory`]). A key table kept from a
    /// read that found the set short is forgotten there, so the next fetch
    /// reads the set again rather than repeating a finding a repair may have
    /// made untrue (spec: KL-15).
    pub fn remembering(mut self, memory: &'a FetchMemory) -> Self {
        self.memory = Some(memory);
        self
    }
}
