use std::collections::BTreeSet;

use coffret_usecase::commit::CommitPolicy;
use coffret_usecase::delete::{
    committed_key_lost, delete_entries, preview_delete, DeleteOutcome, DeletePreview,
    DeleteRequest, DeleteSelection,
};
use coffret_usecase::Progress;
use tracing::info;

use crate::batch_id::next_batch_id;
use crate::device_time::now;
use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Takes the Entries `selection` names out of the Library, in one batch
    /// (spec: PK-9, CP-1).
    ///
    /// Containers every Entry of which is named are removed; Packs that keep
    /// other Entries are rebuilt around them by read-modify-replace
    /// (spec: PK-10); a Pack that keeps Entries and whose key is lost, or that
    /// does not verify when it is read back, is refused and left whole
    /// (spec: PK-10, KL-17). The outcome says which, and a refusal is not an error:
    /// the rest of the deletion commits.
    ///
    /// No mapped folder is touched, and this answers only what leaves the
    /// Library. What becomes of a local file whose Entry left is the next
    /// sync's to decide (spec: EP-15).
    pub async fn delete(
        &self,
        selection: DeleteSelection,
        progress: &dyn Progress,
    ) -> Result<DeleteOutcome> {
        let now = now();
        let batch = next_batch_id(now);

        // Counted rather than named: the paths are the user's own names for
        // their files (spec: EL-1).
        info!(
            operation = "delete",
            library = %self.library_id,
            batch = %batch,
            paths = selection.paths.len(),
            folder = selection.folder.is_some(),
            "deleting Entries from the Library"
        );
        let request = DeleteRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            self.local_fs.as_ref(),
            &self.spool,
            selection,
            batch,
            now,
        )
        .watched_by(progress);
        let outcome = delete_entries(request).await?;
        // The Containers the deletion removed or replaced are out of the
        // current set, and so are any parcels this device kept of them
        // (spec: PK-21).
        self.let_go_parcels().await;
        Ok(outcome)
    }

    /// Counts what [`delete`](Self::delete) would do with `selection`, naming
    /// the Packs it would be refused for.
    ///
    /// The run's own plan over the catalog, after the run's own first step:
    /// the catalog is caught up and the committed Keyring read for the
    /// Containers whose key is lost (spec: CK-9, KL-7). So it reaches Storage
    /// and takes the keys, and writes nothing to the Library.
    pub async fn preview_delete(&self, selection: &DeleteSelection) -> Result<DeletePreview> {
        let key_lost = committed_key_lost(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            &CommitPolicy::default(),
        )
        .await?;
        Ok(preview_delete(self.index.as_ref(), selection, &key_lost).await?)
    }

    /// Whether `selection` names any Entry the catalog holds as current.
    ///
    /// Out of the catalog as it stands and nothing else — no Storage, no key —
    /// for a caller that wants to refuse a deletion of nothing before arming
    /// one.
    pub async fn deletes_anything(&self, selection: &DeleteSelection) -> Result<bool> {
        let plan = preview_delete(self.index.as_ref(), selection, &BTreeSet::new()).await?;
        Ok(plan.entries > 0 || !plan.refused.is_empty())
    }
}
