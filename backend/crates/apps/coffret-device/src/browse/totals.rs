use std::collections::BTreeSet;

use coffret_model::{EntryPath, Mtime};
use tracing::debug;

use crate::error::Result;
use crate::open_library::OpenLibrary;

/// What the whole Library comes to, as the catalog holds it.
///
/// The Library root's own counterpart of a [`ChildFolder`](super::ChildFolder)'s
/// counts, over every Entry rather than one subtree, and the one count no
/// folder has: how many objects the Library keeps on Storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryTotals {
    /// How many objects the Library's current state rests on, as Storage
    /// counts objects — one per name (spec: LA-39): every current Container,
    /// a Pack being one, the control head, and each replica of the committed
    /// Keyring.
    pub objects: u64,
    /// How many Entries the Library holds, at every depth.
    pub files_total: u64,
    /// How many of those this device has, by the present materialization
    /// records a folder's count reads (spec: EP-10).
    pub files_present: u64,
    /// The plaintext bytes of every Entry together, whether this device has
    /// them or not.
    pub size: u64,
    /// The latest modification time among every Entry, and `None` for a
    /// Library holding none.
    pub mtime: Option<Mtime>,
}

impl OpenLibrary {
    /// What the whole Library comes to, out of the catalog alone (spec: LA-39).
    ///
    /// Three reads of the catalog and none of Storage: every Entry, which of
    /// them this device has, and the Containers and the checkpoint the counted
    /// objects are read off. Journal records behind the head, ordinary
    /// checkpoints, and Keyring generations a `prune` has yet to dispose of
    /// are not counted: whether each is still on Storage is something only a
    /// listing of Storage could say, and this asks Storage nothing.
    pub async fn totals(&self) -> Result<LibraryTotals> {
        let entries = self.index.entries_under(None).await?;
        let present: BTreeSet<EntryPath> = self
            .index
            .present_under(None)
            .await?
            .into_iter()
            .map(|local| local.observation.path)
            .collect();
        let containers = self.index.containers_under(None).await?.len() as u64;
        // The head and the Keyring replicas it names: a catalog standing at
        // no committed state names neither, so its Containers are all it
        // counts (spec: LA-39).
        let control = match self.index.checkpoint().await? {
            Some(checkpoint) => 1 + u64::from(checkpoint.keyring().replica_count()),
            None => 0,
        };

        let mut totals = LibraryTotals {
            objects: containers + control,
            files_total: 0,
            files_present: 0,
            size: 0,
            mtime: None,
        };
        for location in &entries {
            totals.files_total += 1;
            totals.files_present += u64::from(present.contains(location.path()));
            totals.size += location.entry.extent.size();
            totals.mtime = totals.mtime.max(Some(location.entry.mtime));
        }
        debug!(
            operation = "totals",
            library = %self.library_id,
            objects = totals.objects,
            files = totals.files_total,
            present = totals.files_present,
            "counted the whole Library",
        );
        Ok(totals)
    }
}
