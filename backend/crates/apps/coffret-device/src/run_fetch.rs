use coffret_model::EntryPath;
use coffret_usecase::fetch::{fetch_folders, FetchOutcome, FetchRequest};
use tracing::info;

use crate::batch_id::now;
use crate::error::Result;
use crate::open_library::{open_library, OpenLibrary};

impl OpenLibrary {
    /// Puts the Library into the folders this device maps, under `prefix`.
    ///
    /// The mirror of [`sync`](Self::sync), and the half of the round trip that
    /// makes a second device worth enrolling: it catches this device's catalog
    /// up to the Library's head — from nothing, on a device that has just
    /// joined — and writes the files it is missing into place (spec: CK-9,
    /// EP-10).
    ///
    /// `prefix` narrows the run to one subtree and never widens it; `None` is
    /// everything the mappings cover (spec: EP-9).
    ///
    /// There is no batch id and no spool: a fetch commits nothing, and it writes
    /// its temporary file into the destination directory, because the rename
    /// that makes a verified file visible has to happen within one filesystem
    /// (spec: EP-11).
    ///
    /// The outcome is not a count to glance at. A folder is a copy of its part
    /// of the Library only where nothing was surfaced: every Entry the run
    /// declined is a path it could not vouch for and left exactly as it was, so
    /// [`Findings`](crate::Findings) over what comes back is the other half of
    /// reading it (spec: EP-11, KL-7).
    pub async fn fetch(&self, prefix: Option<EntryPath>) -> Result<FetchOutcome> {
        info!(
            operation = "fetch",
            library = %self.library_id,
            "fetching into the mapped folders"
        );
        let mut request =
            FetchRequest::new(self.store.as_ref(), self.index.as_ref(), &self.keys, now());
        if let Some(prefix) = prefix {
            request = request.under(prefix);
        }

        Ok(fetch_folders(request).await?)
    }
}

/// Puts the Library called `name` into the folders this device maps, under
/// `prefix`.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it calls
/// [`OpenLibrary::fetch`] and reaches the same body.
pub async fn run_fetch<P>(
    name: &str,
    enter_passphrase: P,
    prefix: Option<EntryPath>,
) -> Result<FetchOutcome>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
{
    open_library(name, enter_passphrase)
        .await?
        .fetch(prefix)
        .await
}
