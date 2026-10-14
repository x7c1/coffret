use std::collections::BTreeSet;

use coffret_model::{ContainerId, EntryPath};
use coffret_usecase::fetch::{read_fronts, FrontsRequest};
use tracing::debug;

use crate::error::Result;
use crate::open_library::OpenLibrary;

/// How many of a listed folder's Containers have their fronts read ahead.
///
/// A book is one Pack, so a folder of pages is one or two Containers and the
/// bound is never what stops it there. What it is for is a folder of loose
/// files, each its own Container: reading every front of one of those would be
/// a round of requests per file for a listing nobody may open anything from.
/// The first ones in listing order are the ones a reader opens first.
pub const FRONTS_PER_LISTING: usize = 8;

impl OpenLibrary {
    /// Reads the fronts of the Containers the Entries of `folder` this device
    /// does not have live in, and keeps them in
    /// [`fetch_memory`](Self::fetch_memory) — the first
    /// [`FRONTS_PER_LISTING`] distinct ones in listing order, and only those
    /// not kept already.
    ///
    /// What a process serving a reader does as somebody lists a folder, so the
    /// first page they open costs its parcels and not the two reads of a front
    /// before them (spec: FM-2, PK-16). The front names no Entry, so what the
    /// provider sees of it is the folder's Containers being opened, which the
    /// fill a page opens would show it anyway (spec: PK-20). The same Storage,
    /// the same memory, and the same listing and key table the Entry fetch
    /// answering from the catalog as it stands uses, so what is read here is
    /// what that fetch would have read.
    ///
    /// Answers with how many fronts were read. A folder wholly on this device,
    /// or whose fronts are all kept, reads nothing of Storage.
    pub async fn read_fronts_of(&self, folder: Option<&EntryPath>) -> Result<usize> {
        let mut seen = BTreeSet::new();
        let containers: Vec<ContainerId> = self
            .remote_in(folder)
            .await?
            .into_iter()
            .map(|location| location.container_id)
            .filter(|container_id| seen.insert(*container_id))
            .take(FRONTS_PER_LISTING)
            .collect();
        if containers.is_empty() {
            debug!(
                operation = "read_fronts",
                library = %self.library_id,
                "nothing in the listed folder is to be fetched, so no front is read",
            );
            return Ok(0);
        }
        let request = FrontsRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            &self.fetch_memory,
        );
        Ok(read_fronts(request, folder, &containers).await?)
    }
}
