use std::collections::BTreeSet;

use coffret_model::EntryPath;
use tracing::debug;

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Every folder in the Library, in EP-3 order.
    ///
    /// Flat and complete: each folder is named by its whole path, and a folder
    /// implied by an Entry Path several components deep contributes every
    /// ancestor along the way. Whatever nests them into a tree does so from
    /// this, which is what keeps the nesting out of the catalog — where there
    /// are no folders to nest.
    ///
    /// A folder exists exactly where a current Entry stands under it. There are
    /// no empty ones, because a Library holds Entries and nothing else: a folder
    /// whose last Entry is removed stops existing at that commit.
    pub async fn folders(&self) -> Result<Vec<EntryPath>> {
        let entries = self.index.entries_under(None).await?;

        // Byte order over an Entry Path is EP-3 order, which is the order the
        // answer owes, so the set both dedupes the ancestors and sorts them.
        //
        // Each ancestor is taken off the path a whole component at a time rather
        // than cut out of its text: what is left of an Entry Path when a
        // trailing component is dropped is an Entry Path, so nothing here has to
        // make one out of text and there is no refusal to invent an answer for
        // about text the Library itself produced (spec: EP-1, EP-2).
        let mut folders: BTreeSet<EntryPath> = BTreeSet::new();
        for location in &entries {
            let mut folder = location.path().parent();
            while let Some(above) = folder {
                folder = above.parent();
                if !folders.insert(above) {
                    // Everything above it is in the set already, put there by
                    // whichever Entry first stood under it.
                    break;
                }
            }
        }

        debug!(
            entries = entries.len(),
            folders = folders.len(),
            "derived the Library's folders from the catalog",
        );
        Ok(folders.into_iter().collect())
    }
}
