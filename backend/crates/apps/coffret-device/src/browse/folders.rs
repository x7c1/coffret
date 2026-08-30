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

        // Byte order over `&str` is EP-3 order, which is the order the answer
        // owes, so the set both dedupes the ancestors and sorts them.
        let mut folders: BTreeSet<&str> = BTreeSet::new();
        for location in &entries {
            let path = location.path().as_str();
            let mut cut = 0;
            while let Some(at) = path[cut..].find('/') {
                cut += at;
                folders.insert(&path[..cut]);
                cut += 1;
            }
        }

        debug!(
            entries = entries.len(),
            folders = folders.len(),
            "derived the Library's folders from the catalog",
        );
        // Every one of these is a prefix of an Entry Path the catalog answered
        // with, cut at a separator, so it is already in the Library's spelling
        // and `nfc` is the identity on it (spec: EP-1). It is used rather than
        // `stored` so that deriving a folder from a path has no failure mode to
        // report about text the Library itself produced.
        Ok(folders.into_iter().map(EntryPath::nfc).collect())
    }
}
