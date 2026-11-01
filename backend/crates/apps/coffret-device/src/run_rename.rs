use std::collections::BTreeSet;

use coffret_model::{EntryPath, Passphrase};
use coffret_usecase::rename::{
    preview_rename, rename_entries, RenameOutcome, RenamePair, RenameRequest,
};
use coffret_usecase::Progress;
use tracing::info;

use crate::device_time::now;
use crate::error::Result;
use crate::library_ref::LibraryRef;
use crate::move_preview::MovePreview;
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
    /// would relocate them; one that cannot follow leaves this device,
    /// discarded into the trash where it is unedited and kept where it is
    /// edited, each reported in the outcome's relocations (spec: EP-10).
    /// Other devices' files follow on their next sync or fetch.
    ///
    /// `progress` is where the run says it is catching up and committing, for a
    /// caller with somewhere to show it.
    pub async fn rename(
        &self,
        from: EntryPath,
        to: EntryPath,
        progress: &dyn Progress,
    ) -> Result<RenameOutcome> {
        self.rename_pairs(vec![RenamePair::new(from, to)], progress)
            .await
    }

    /// Renames every pair of `pairs` in one Journal record, as
    /// [`rename`](Self::rename) renames one, refusing the whole set where any
    /// pair is refused or two of them overlap (spec: FM-15, CP-1, EP-18).
    ///
    /// A pair whose source is under another pair's source is one move of that
    /// folder. This device's files of the renamed Entries follow them, across
    /// mapped folders on one filesystem too; one that cannot follow is
    /// discarded into the trash where it is unedited, and kept where it is
    /// edited, each reported in the outcome's relocations (spec: EP-10).
    pub async fn rename_pairs(
        &self,
        pairs: Vec<RenamePair>,
        progress: &dyn Progress,
    ) -> Result<RenameOutcome> {
        // Counts rather than the paths: they are the user's own names for
        // their files (spec: EL-1).
        info!(
            operation = "rename",
            library = %self.library_id,
            pairs = pairs.len(),
            "renaming in the Library"
        );
        let outcome = rename_entries(self.rename_request(pairs).watched_by(progress)).await;
        // The commit selects a Keyring generation of its own, so the key table
        // a later read holds is that one's, read afresh.
        self.fetch_memory.forget_key_table();
        Ok(outcome?)
    }

    /// Works out what moving every pair of `pairs` would do, and writes
    /// nothing: the rename's plan over the catalog caught up to the head, what
    /// this device's files of the renamed Entries would do — follow them, or
    /// leave this device — which of them are edited, and which files under the
    /// set the Library does not hold yet (spec: FM-15, EP-10).
    ///
    /// It reaches Storage to catch up, and reads this device's mapped folders.
    pub async fn preview_move(&self, pairs: &[RenamePair]) -> Result<MovePreview> {
        let rename = preview_rename(self.rename_request(pairs.to_vec())).await?;
        let uploading = if rename.refused.is_some() {
            Vec::new()
        } else {
            self.uploading_under(pairs, &rename.renamed).await?
        };
        Ok(MovePreview { rename, uploading })
    }

    /// The files under any pair's source that this device has and the Library
    /// does not hold yet: reserved for a Pack, or standing in a mapped folder
    /// with no Entry at their path (spec: PK-17, EP-10).
    ///
    /// The folders asked about are every folder of the Library at or under a
    /// source, and every folder standing on disk under those that the Library
    /// has none of.
    async fn uploading_under(
        &self,
        pairs: &[RenamePair],
        renamed: &[coffret_usecase::rename::RenamedPath],
    ) -> Result<Vec<EntryPath>> {
        let under_a_source = |path: &EntryPath| pairs.iter().any(|pair| path.is_under(&pair.from));
        let mut uploading: BTreeSet<EntryPath> = self
            .reserved_paths()
            .await?
            .into_iter()
            .filter(|path| under_a_source(path))
            .collect();

        // The folders of the Library under a source, the source itself among
        // them where it is one: a source that is a file has no folder to ask.
        let mut folders: BTreeSet<EntryPath> = BTreeSet::new();
        for one in renamed {
            let mut above = one.from.parent();
            while let Some(folder) = above {
                if !under_a_source(&folder) {
                    break;
                }
                above = folder.parent();
                folders.insert(folder);
            }
        }
        let mut asked: BTreeSet<EntryPath> = BTreeSet::new();
        while let Some(folder) = folders.pop_first() {
            if !asked.insert(folder.clone()) {
                continue;
            }
            let added = self.added_locally(Some(&folder)).await?;
            uploading.extend(added.files.into_iter().map(|file| file.path));
            // A name a folder on disk carries that is no Entry Path holds
            // nothing a sync carries in either.
            for name in added.folders {
                if let Ok(name) = EntryPath::parse(&name) {
                    folders.insert(folder.below(&name));
                }
            }
        }
        Ok(uploading.into_iter().collect())
    }

    /// The request every rename and its preview are made from: this device's
    /// Library, its keys and spool, and its disk for the relocations.
    fn rename_request(&self, pairs: Vec<RenamePair>) -> RenameRequest<'_> {
        RenameRequest::of_pairs(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            self.local_fs.as_ref(),
            pairs,
            now(),
        )
        .with_local_disk(
            self.local_fs.as_ref(),
            self.local_fs.as_ref(),
            self.trash.as_ref(),
        )
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
