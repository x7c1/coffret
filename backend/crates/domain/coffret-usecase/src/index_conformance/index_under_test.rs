use crate::index::Index;
use crate::index_conformance::stored_form::StoredForm;

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
    stored_form: Option<Box<dyn StoredForm>>,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl IndexUnderTest {
    /// Takes two empty, independent catalogs.
    pub fn new(index: Box<dyn Index>, other: Box<dyn Index>) -> Self {
        Self {
            index,
            other,
            stored_form: None,
            resources: Vec::new(),
        }
    }

    /// Hands over a way to write into [`index`](Self::index)'s stored form
    /// past the port.
    pub fn with_stored_form(mut self, stored_form: Box<dyn StoredForm>) -> Self {
        self.stored_form = Some(stored_form);
        self
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

    /// The way into [`index`](Self::index)'s stored form, where the
    /// implementation has one to hand over.
    pub fn stored_form(&self) -> Option<&dyn StoredForm> {
        self.stored_form.as_deref()
    }
}
