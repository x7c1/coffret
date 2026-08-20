use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;

/// A listing read to its end, kept page by page.
///
/// The cases care about more than the set of objects: that the walk terminated,
/// how many pages it took, and whether any object turned up twice are all part
/// of the contract, so the pages are kept rather than flattened away.
pub struct ListingWalk {
    pages: Vec<ObjectPage>,
}

/// How many pages a walk may take before the suite calls the listing broken.
///
/// A provider that keeps handing back a token that makes no progress would
/// otherwise hang the test run instead of failing it.
const MAX_PAGES: usize = 1000;

impl ListingWalk {
    /// Reads a listing from its first page to its last.
    pub async fn read(store: &dyn ObjectStore) -> Self {
        let mut pages: Vec<ObjectPage> = Vec::new();
        let mut token = None;
        loop {
            let page = store
                .list(token.as_ref())
                .await
                .expect("listing a store must succeed");

            token = page.next.clone();
            pages.push(page);

            if token.is_none() {
                break;
            }
            assert!(
                pages.len() < MAX_PAGES,
                "listing did not end within {MAX_PAGES} pages"
            );
        }
        Self { pages }
    }

    /// How many pages the walk took.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// The names the walk reported, in the order they arrived.
    pub fn names(&self) -> Vec<String> {
        self.pages
            .iter()
            .flat_map(|page| page.objects.iter())
            .map(|object| object.name.clone())
            .collect()
    }

    /// The names the walk reported, sorted, after asserting none repeated.
    ///
    /// A duplicate means pagination re-served a page, which would make a caller
    /// scanning Storage count the same object twice.
    pub fn distinct_names(&self) -> Vec<String> {
        let mut names = self.names();
        let seen = names.len();
        names.sort();
        names.dedup();
        assert_eq!(seen, names.len(), "pagination served an object twice");
        names
    }
}
