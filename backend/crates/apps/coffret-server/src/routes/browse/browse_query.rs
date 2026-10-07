use serde::Deserialize;

/// The `?path=` a browse was asked with: a whole path on this device, or none
/// for the home directory.
///
/// Not a [`PathQuery`](crate::entry_query::PathQuery): that one names a place
/// in the Library, and this names a folder on this device's own disk, which is
/// a different namespace with different rules (spec: EP-9).
#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub(super) path: Option<String>,
}
