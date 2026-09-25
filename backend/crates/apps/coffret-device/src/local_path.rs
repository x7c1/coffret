use std::path::PathBuf;

use coffret_model::EntryPath;
use coffret_usecase::fetch::local_path_of;

use crate::error::{Error, Result};
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Where on this device the file for the Entry at `path` belongs
    /// (spec: EP-9).
    ///
    /// This joined path is for display and reporting. A shell asks this call
    /// rather than rederiving EP-9 from the mappings itself. Reading the file uses
    /// [`open_local_file`](Self::open_local_file), which keeps the mapped root and
    /// validated relative location separate during descriptor descent
    /// (spec: EP-8).
    ///
    /// It is where the file *belongs* and never a claim that it is there:
    /// [`state_of`](Self::state_of) is what says whether this device has it
    /// (spec: EP-10).
    ///
    /// # Errors
    ///
    /// [`Error::LocalPathNotSettled`](crate::Error::LocalPathNotSettled)
    /// carrying `EntryNotCurrent` where the Library holds no current Entry at
    /// the path, `UnmappedEntryPath` where it holds one that no mapping of this
    /// device reaches, `UnmaterializablePath` where a mapping does reach it and
    /// no file here can stand for it (spec: EP-2, EP-4). A shell telling one of
    /// these from another does so by the [`FetchError`](crate::FetchError) it
    /// carries, which is why this crate re-exports that type.
    ///
    /// [`Error::Index`](crate::Error::Index) where the catalog could not be
    /// read at all, which settled nothing about the path and is reported the
    /// way every other entry point reports it.
    ///
    /// Not `Fetch`, although the vocabulary inside it is the fetch's: no fetch
    /// was begun here, and a caller printing the chain of one is owed an outer
    /// sentence about the question it actually asked.
    pub async fn local_path_of(&self, path: &EntryPath) -> Result<PathBuf> {
        local_path_of(self.index.as_ref(), path)
            .await
            .map_err(Error::local_path_not_settled)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use coffret_model::{LibraryId, MasterKey, MasterKeyEpoch};
    use coffret_usecase::fetch::FetchError;
    use coffret_usecase::{InMemoryIndex, InMemoryStore, LibraryKeys};

    use crate::error::Error;
    use crate::open_library::OpenLibrary;
    use crate::testing::{entry_path, local_fs};

    /// A device whose catalog holds nothing, which is the whole of what this
    /// case needs: the translation refuses before it reads a mapping, and which
    /// refusal it is, is not what is being asked here.
    fn device() -> OpenLibrary {
        OpenLibrary {
            store: Arc::new(InMemoryStore::new(64)),
            index: Arc::new(InMemoryIndex::new()),
            local_fs: local_fs(),
            keys: LibraryKeys::derive(
                &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
                MasterKeyEpoch::FIRST,
            ),
            spool: std::env::temp_dir(),
            library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
            epoch: MasterKeyEpoch::FIRST,
            provider: "s3",
        }
    }

    // The call answers in the fetch's vocabulary without being a fetch, so the
    // crate has a conversion from that vocabulary standing ready — and it makes
    // `Error::Fetch`. A `?` here would take it, compile, and quietly move the
    // outer sentence of every chain this call hands out; the error type's own
    // case pins that sentence on a value built by hand, and this one pins which
    // value the call builds.
    #[tokio::test]
    async fn asking_where_a_file_belongs_is_not_answered_as_a_fetch() {
        let result = device()
            .local_path_of(&entry_path("albums/spring.jpg"))
            .await;

        assert!(
            matches!(
                &result,
                Err(Error::LocalPathNotSettled { cause })
                    if matches!(**cause, FetchError::EntryNotCurrent { .. }),
            ),
            "expected the question the caller asked to be the one refused, got {result:?}",
        );
    }
}
