use crate::object_store::ObjectStore;

/// What a gateway hands the conformance suite for one case.
///
/// Every case starts from an empty store, so a gateway's fixture points each
/// one at storage nothing else is writing to — a fresh key prefix, a fresh
/// folder. The page size comes along because the pagination case has to write
/// more objects than one page holds, and only the gateway knows how many that
/// is.
pub struct StoreUnderTest {
    store: Box<dyn ObjectStore>,
    page_size: usize,
}

impl StoreUnderTest {
    /// Takes an empty store and the number of objects one of its listing pages
    /// holds.
    ///
    /// # Panics
    ///
    /// If `page_size` is zero: a listing whose pages hold nothing never
    /// finishes, so the suite would hang rather than fail.
    pub fn new(store: Box<dyn ObjectStore>, page_size: usize) -> Self {
        assert!(
            page_size > 0,
            "a listing page must hold at least one object"
        );
        Self { store, page_size }
    }

    /// The store the case exercises.
    pub fn store(&self) -> &dyn ObjectStore {
        self.store.as_ref()
    }

    /// How many objects one listing page of this store holds.
    pub fn page_size(&self) -> usize {
        self.page_size
    }
}
