use coffret_model::{EntryPath, Passphrase};
use coffret_usecase::rename::{rename_entries, RenameOutcome, RenameRequest};
use coffret_usecase::Progress;
use tracing::info;

use crate::device_time::now;
use crate::error::Result;
use crate::library_ref::LibraryRef;
use crate::open_library::{open_library, OpenLibrary};

impl OpenLibrary {
    /// Gives the Entry at `from`, or every Entry under the folder `from`, a
    /// new Entry Path under `to`, in one Journal record (spec: FM-15, CP-1).
    ///
    /// Nothing is read, encrypted or uploaded: the record names each Entry by
    /// its Container and entry number, and the Containers stay exactly as they
    /// are. A rename the Library does not admit — nothing at `from`, a
    /// destination another Entry holds, a path it would leave both a file and a
    /// folder — is refused before anything is written, as
    /// [`RenameError::Refused`](coffret_usecase::rename::RenameError::Refused).
    ///
    /// A batch of this device's whose record the catch-up finds committed and
    /// whose refresh never ran is completed first, so the files it carried in
    /// are this device's own when the rename relocates (spec: OC-7).
    ///
    /// Once the record is committed, this device's own files of the renamed
    /// Entries follow them in its mapped folders, as the next sync or fetch
    /// would relocate them; one that cannot follow is reported in the outcome's
    /// relocations and stays where it was (spec: EP-10). Other devices'
    /// files follow on their next sync or fetch.
    ///
    /// `progress` is where the run says it is catching up and committing, for a
    /// caller with somewhere to show it.
    pub async fn rename(
        &self,
        from: EntryPath,
        to: EntryPath,
        progress: &dyn Progress,
    ) -> Result<RenameOutcome> {
        // Lengths rather than the paths: they are the user's own names for
        // their files (spec: EL-1).
        info!(
            operation = "rename",
            library = %self.library_id,
            from_len = from.as_str().len(),
            to_len = to.as_str().len(),
            "renaming in the Library"
        );
        let outcome = rename_entries(
            RenameRequest::new(
                self.store.as_ref(),
                self.index.as_ref(),
                &self.keys,
                self.local_fs.as_ref(),
                from,
                to,
                now(),
            )
            .watched_by(progress)
            .with_local_disk(
                self.local_fs.as_ref(),
                self.local_fs.as_ref(),
                self.local_fs.as_ref(),
            ),
        )
        .await;
        // The commit selects a Keyring generation of its own, so the key table
        // a later read holds is that one's, read afresh.
        self.fetch_memory.forget_key_table();
        Ok(outcome?)
    }
}

/// Renames `from` to `to` in the Library `library` names.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it calls
/// [`OpenLibrary::rename`] and reaches the same body.
pub async fn run_rename<P>(
    library: &LibraryRef,
    enter_passphrase: P,
    from: EntryPath,
    to: EntryPath,
    progress: &dyn Progress,
) -> Result<RenameOutcome>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    open_library(library, enter_passphrase)
        .await?
        .rename(from, to, progress)
        .await
}
