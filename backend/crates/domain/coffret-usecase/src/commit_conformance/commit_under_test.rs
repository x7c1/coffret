use crate::index::Index;
use crate::object_store::ObjectStore;

/// What a backend hands the commit suite for one case.
///
/// One store and two catalogs, and both halves of that matter. The store is
/// where the Library actually is, so a case that asserts what a commit left
/// behind asserts it against the same Storage a device would read; the second
/// catalog is a second device, which is the only way to drive two writers
/// racing for one commit slot (spec: CP-3) and a rebase onto a head somebody
/// else committed (spec: CP-4).
///
/// Every case starts from a store holding nothing and two catalogs standing at
/// no committed state. The catalogs must not share storage: two devices that
/// did would agree by accident rather than because a Snapshot and a record say
/// the same thing.
pub struct CommitUnderTest {
    // Dropped before `resources`, so that whatever a catalog or a store is kept
    // in outlives them.
    store: Box<dyn ObjectStore>,
    index: Box<dyn Index>,
    other: Box<dyn Index>,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl CommitUnderTest {
    /// Takes an empty store and two empty, independent catalogs.
    pub fn new(store: Box<dyn ObjectStore>, index: Box<dyn Index>, other: Box<dyn Index>) -> Self {
        Self {
            store,
            index,
            other,
            resources: Vec::new(),
        }
    }

    /// Keeps something alive for as long as the case runs.
    ///
    /// A backend that puts its catalogs in a temporary directory, or its
    /// Library under a key prefix it wants cleaned up, hands the owner over here
    /// rather than leaking it.
    pub fn holding(mut self, resource: Box<dyn Send + Sync>) -> Self {
        self.resources.push(resource);
        self
    }

    /// The Storage the Library lives in.
    pub fn store(&self) -> &dyn ObjectStore {
        self.store.as_ref()
    }

    /// The catalog of the device a case commits from.
    pub fn index(&self) -> &dyn Index {
        self.index.as_ref()
    }

    /// The second device's catalog, for the cases that need two writers.
    pub fn other(&self) -> &dyn Index {
        self.other.as_ref()
    }
}
