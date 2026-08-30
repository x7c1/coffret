use coffret_model::EntryPath;
use coffret_usecase::freeze::{freeze_folder, FreezeOutcome, FreezeRequest};
use tracing::info;

use crate::batch_id::{next_batch_id, now};
use crate::error::Result;
use crate::open_library::{open_library, OpenLibrary};

impl OpenLibrary {
    /// Packs the eligible files under `prefix` into Packs of about `target`
    /// bytes each.
    ///
    /// `prefix` narrows the run to one top-level folder of the Library and never
    /// widens it; `None` is everything the mappings cover (spec: PK-17, EP-9).
    ///
    /// `target` is a parameter and not a constant, and it is one deliberately:
    /// what size serves best is a measurement question about upload and
    /// retrieval behaviour, rewrite amplification, object count and provider API
    /// overhead (spec: PK-5, PK-6). A Library can be repacked under a different
    /// one, so nothing in the byte forms may come to depend on today's answer.
    ///
    /// The outcome is not a count to glance at: a file whose Entry an existing
    /// Pack holds is reported rather than repacked, and so is one whose Pack the
    /// Library records no key for, so [`Findings`](crate::Findings) over what
    /// comes back is the other half of reading it (spec: PK-14, PK-11).
    pub async fn freeze(&self, prefix: Option<EntryPath>, target: u64) -> Result<FreezeOutcome> {
        let now = now();
        let batch = next_batch_id(now);

        // The target is the one decision this run makes that another run of the
        // same folder could make differently, so it is what the log keeps about
        // it. The batch id names the spool an interrupted run leaves behind
        // (spec: OC-2).
        info!(
            operation = "freeze",
            library = %self.library_id,
            batch = %batch,
            target,
            "freezing the mapped folders"
        );
        let mut request = FreezeRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            &self.spool,
            target,
            batch,
            now,
        );
        if let Some(prefix) = prefix {
            request = request.under(prefix);
        }

        Ok(freeze_folder(request).await?)
    }
}

/// Packs the eligible files under `prefix` of the Library called `name` into
/// Packs of about `target` bytes each.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it calls
/// [`OpenLibrary::freeze`] and reaches the same body.
pub async fn run_freeze<P>(
    name: &str,
    enter_passphrase: P,
    prefix: Option<EntryPath>,
    target: u64,
) -> Result<FreezeOutcome>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
{
    open_library(name, enter_passphrase)
        .await?
        .freeze(prefix, target)
        .await
}
