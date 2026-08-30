use coffret_model::EntryPath;
use coffret_usecase::fetch::{fetch_entry, EntryFetch, FetchEntryRequest};
use tracing::info;

use crate::batch_id::now;
use crate::error::Result;
use crate::open_library::{open_library, OpenLibrary};

impl OpenLibrary {
    /// Puts the one Entry at `path` into the folder this device maps it to.
    ///
    /// The same journey [`fetch`](Self::fetch) makes, with the read done
    /// differently: a Container says where everything in it is before any of it
    /// arrives, so one Entry costs the front of the object and the chunks
    /// covering that Entry rather than the gigabyte around it (spec: FM-2,
    /// FM-5, FM-9).
    ///
    /// Per PK-16 that is an optimization inside fetching the containing
    /// Container and not a fetch unit of its own: the rest of the Container is
    /// exactly as unfetched afterwards. So this is what a reader wanting one
    /// page uses, and never what a device bringing a folder into line uses.
    ///
    /// The answer is one of three, and the third is a finding rather than a
    /// failure: the Entry was placed, it was already here, or the run declined
    /// the path and says why (spec: EP-11).
    ///
    /// Nothing here keeps two callers asking for one Entry from both running it.
    /// A process that serves more than one reader wants
    /// [`EntryFetches`](crate::EntryFetches) around this, which is where that
    /// belongs: it is a property of the process rather than of the Library.
    pub async fn fetch_entry(&self, path: EntryPath) -> Result<EntryFetch> {
        // The Entry Path is not in the event and never will be: it is the user's
        // own name for their file (spec: EP-1), and a log is not where that
        // goes.
        info!(
            operation = "fetch_entry",
            library = %self.library_id,
            "fetching one Entry"
        );
        Ok(fetch_entry(FetchEntryRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            path,
            now(),
        ))
        .await?)
    }
}

/// Puts the one Entry at `path` of the Library called `name` into the folder
/// this device maps it to.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it calls
/// [`OpenLibrary::fetch_entry`] and reaches the same body.
pub async fn run_fetch_entry<P>(
    name: &str,
    enter_passphrase: P,
    path: EntryPath,
) -> Result<EntryFetch>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
{
    open_library(name, enter_passphrase)
        .await?
        .fetch_entry(path)
        .await
}
