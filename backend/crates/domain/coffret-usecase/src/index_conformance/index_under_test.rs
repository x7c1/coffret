use crate::index::Index;

/// What an adapter hands the conformance suite for one case.
///
/// Every case starts from two catalogs that are empty and independent of each
/// other. The second one is not a spare in the sense of being unused: several
/// pieces of the contract are statements that two ways of reaching one
/// committed Library state agree — a replay against a restore of the head it
/// reaches, this device's own commit against another device's record of it —
/// and a case can only assert that by driving two catalogs and comparing them.
///
/// They must not share storage: an adapter backed by files gives each its own.
pub struct IndexUnderTest {
    // Dropped before `resources`, so that whatever a catalog is kept in outlives
    // the catalog itself.
    index: Box<dyn Index>,
    other: Box<dyn Index>,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl IndexUnderTest {
    /// Takes two empty, independent catalogs.
    pub fn new(index: Box<dyn Index>, other: Box<dyn Index>) -> Self {
        Self {
            index,
            other,
            resources: Vec::new(),
        }
    }

    /// Keeps something alive for as long as the case runs.
    ///
    /// An adapter that puts its catalogs in a temporary directory hands the
    /// directory over here rather than leaking it: the case owns the fixture
    /// for exactly as long as the catalogs are wanted, and the directory goes
    /// away with it.
    pub fn holding(mut self, resource: Box<dyn Send + Sync>) -> Self {
        self.resources.push(resource);
        self
    }

    /// The catalog a case drives.
    pub fn index(&self) -> &dyn Index {
        self.index.as_ref()
    }

    /// The second catalog, for the cases that compare two ways to one state.
    pub fn other(&self) -> &dyn Index {
        self.other.as_ref()
    }
}
