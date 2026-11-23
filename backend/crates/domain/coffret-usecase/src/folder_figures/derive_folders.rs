use std::collections::BTreeSet;

use coffret_model::EntryPath;

use crate::index::Index;
use crate::index_error::IndexResult;

/// Every folder of the Library, in EP-3 order, read off every Entry.
///
/// Each ancestor is taken off a path a whole component at a time rather than
/// cut out of its text: what is left of an Entry Path when a trailing
/// component is dropped is an Entry Path (spec: EP-1, EP-2).
pub async fn derive_folders(index: &(impl Index + ?Sized)) -> IndexResult<Vec<EntryPath>> {
    // Byte order over an Entry Path is EP-3 order, so the set both dedupes the
    // ancestors and sorts them.
    let mut folders: BTreeSet<EntryPath> = BTreeSet::new();
    for location in index.entries_under(None).await? {
        let mut folder = location.path().parent();
        while let Some(above) = folder {
            folder = above.parent();
            if !folders.insert(above) {
                // Everything above it is in the set already.
                break;
            }
        }
    }
    Ok(folders.into_iter().collect())
}
