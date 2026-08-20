use crate::object_info::ObjectInfo;
use crate::page_token::PageToken;

/// One page of a [`list`](crate::ObjectStore::list).
///
/// Listing is always paginated, because no provider promises to return a whole
/// Library in one answer and a caller that assumed otherwise would silently see
/// a truncated Storage. A page with no `next` is the last one; a page can be
/// empty and still carry a `next`, so "walk until `next` is `None`" is the only
/// correct way to read a listing to the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPage {
    /// The objects on this page.
    pub objects: Vec<ObjectInfo>,
    /// Where to resume, or `None` when this page ends the listing.
    pub next: Option<PageToken>,
}

impl ObjectPage {
    /// The last page of a listing.
    pub fn last(objects: Vec<ObjectInfo>) -> Self {
        Self {
            objects,
            next: None,
        }
    }

    /// A page another [`list`](crate::ObjectStore::list) call continues from.
    pub fn resumable(objects: Vec<ObjectInfo>, next: PageToken) -> Self {
        Self {
            objects,
            next: Some(next),
        }
    }
}
