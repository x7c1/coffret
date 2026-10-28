use coffret_model::{EntryPath, Passphrase};
use coffret_usecase::repack::{repack_folder, RepackOutcome, RepackRequest};
use coffret_usecase::Progress;
use tracing::info;

use crate::batch_id::next_batch_id;
use crate::device_time::now;
use crate::error::Result;
use crate::library_ref::LibraryRef;
use crate::open_library::{open_library, OpenLibrary};

impl OpenLibrary {
    /// Rewrites the Packs holding the Entries under `folder` into Packs of
    /// about `target` bytes cut in Entry Path order, in one batch
    /// (spec: PK-8, PK-23).
    ///
    /// `folder` is an Entry Path prefix, and `None` is the Library root — named
    /// on purpose, since a repack of it rewrites every Pack not already in
    /// place. Every Pack holding an Entry under the folder takes part whole;
    /// one-file Containers do not (spec: PK-1). A Pack that is already a
    /// segment of the cut is kept as it is, so a second run straight after a
    /// first commits nothing.
    ///
    /// All or nothing: a Pack to be rewritten whose key is lost, or that does
    /// not verify when it is read, refuses the whole run before anything is
    /// committed, and the refusal names it (spec: PK-10, KL-17).
    ///
    /// Nothing calls this but a person asking for it: no flow repacks after a
    /// rename, a deletion or a freeze.
    pub async fn repack(
        &self,
        folder: Option<EntryPath>,
        target: u64,
        progress: &dyn Progress,
    ) -> Result<RepackOutcome> {
        let now = now();
        let batch = next_batch_id(now);

        // The folder is said only as whether there is one: it is the user's
        // own name for their files (spec: EL-1). The batch id names the spool
        // an interrupted run leaves behind (spec: OC-2).
        info!(
            operation = "repack",
            library = %self.library_id,
            batch = %batch,
            target,
            root = folder.is_none(),
            "repacking a folder's Packs in path order"
        );
        let request = RepackRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            self.local_fs.as_ref(),
            &self.spool,
            folder,
            target,
            batch,
            now,
        )
        .watched_by(progress);
        let outcome = repack_folder(request).await;
        self.fetch_memory.forget_key_table();
        let outcome = outcome?;
        // The Packs the repack replaced are out of the current set, and so are
        // any parcels this device kept of them (spec: PK-21).
        self.let_go_parcels().await;
        Ok(outcome)
    }
}

/// Repacks `folder` of the Library `library` names into Packs of about
/// `target` bytes each.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it calls
/// [`OpenLibrary::repack`] and reaches the same body.
pub async fn run_repack<P>(
    library: &LibraryRef,
    enter_passphrase: P,
    folder: Option<EntryPath>,
    target: u64,
    progress: &dyn Progress,
) -> Result<RepackOutcome>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    open_library(library, enter_passphrase)
        .await?
        .repack(folder, target, progress)
        .await
}
