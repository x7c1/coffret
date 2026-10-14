use std::collections::BTreeSet;

use coffret_model::{EntryLocation, EntryPath};

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// The Entries standing directly in `folder` that this device does not
    /// have, in EP-3 order — the rows a listing calls `remote`.
    pub(crate) async fn remote_in(&self, folder: Option<&EntryPath>) -> Result<Vec<EntryLocation>> {
        let present: BTreeSet<EntryPath> = self
            .index
            .present_under(folder)
            .await?
            .into_iter()
            .map(|local| local.observation.path)
            .collect();
        Ok(self
            .index
            .entries_under(folder)
            .await?
            .into_iter()
            .filter(|location| {
                let path = location.path();
                path.parent().as_ref() == folder && !present.contains(path)
            })
            .collect())
    }
}
