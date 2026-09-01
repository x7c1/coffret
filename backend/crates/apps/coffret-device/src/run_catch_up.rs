use coffret_usecase::catch_up::{catch_up_catalog, CatchUpOutcome, CatchUpRequest};
use tracing::info;

use crate::error::Result;
use crate::open_library::{open_library, OpenLibrary};

impl OpenLibrary {
    /// Brings this device's catalog to the Library's head, and stops there.
    ///
    /// The first step of a [`sync`](Self::sync) and of a [`fetch`](Self::fetch)
    /// on its own (spec: CK-9), for the caller that wants to know what the
    /// Library has become without bringing any of it over: a device that has just
    /// joined and holds nothing, and one whose Library another device has written
    /// to since it last looked.
    ///
    /// It takes no prefix and there is nothing to narrow: a Journal record is
    /// replayed whole, and a catalog caught up under one folder would be a
    /// catalog standing at no committed state at all.
    ///
    /// Nothing on this device's disk changes. Every Entry the run learns of is
    /// `remote` until something asks for its bytes (spec: EP-10), which is what
    /// makes this the cheap question — the control objects and nothing else.
    pub async fn catch_up(&self) -> Result<CatchUpOutcome> {
        info!(
            operation = "catch_up",
            library = %self.library_id,
            "catching the catalog up with the Library's head"
        );
        Ok(catch_up_catalog(CatchUpRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
        ))
        .await?)
    }
}

/// Brings the catalog of the Library called `name` to its head.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it — the
/// explorer's server, which catches up as it starts and again whenever somebody
/// asks what is new — calls [`OpenLibrary::catch_up`] and reaches the same body.
pub async fn run_catch_up<P>(name: &str, enter_passphrase: P) -> Result<CatchUpOutcome>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
{
    open_library(name, enter_passphrase).await?.catch_up().await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use coffret_model::{LibraryId, MasterKey, MasterKeyEpoch};
    use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping};
    use coffret_usecase::sync::{sync_folders, SyncRequest};
    use coffret_usecase::{InMemoryIndex, InMemoryStore, Index, LibraryKeys, ObjectStore};

    use crate::open_library::OpenLibrary;

    /// The Master Key both devices work under.
    fn keys() -> LibraryKeys {
        LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        )
    }

    /// One device over `store`, whose catalog is `index`.
    ///
    /// The spool is named and never made, which is the point: a catch-up spools
    /// nothing, so a directory that comes into existence would be this flow doing
    /// something it may not.
    fn device(store: &Arc<dyn ObjectStore>, index: Arc<dyn Index>) -> OpenLibrary {
        OpenLibrary {
            store: Arc::clone(store),
            index,
            keys: keys(),
            spool: std::env::temp_dir().join("coffret-a-catch-up-never-spools"),
            library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
            epoch: MasterKeyEpoch::FIRST,
            provider: "s3",
        }
    }

    // The whole of what a second device gets out of this: a record the first one
    // committed reaches its catalog, and nothing else happens. No fetch, no sync,
    // and not one byte in the folder it maps — the rows arrive `remote`, and the
    // files behind them stay where they are until somebody asks for one
    // (spec: CK-9, EP-10).
    #[tokio::test]
    async fn a_second_device_catches_up_with_what_the_first_one_committed() {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new(64));
        let theirs = tempfile::tempdir().expect("a temporary directory must be available");
        let mine = tempfile::tempdir().expect("a temporary directory must be available");

        // The device that has the folder, carrying it into the Library.
        std::fs::create_dir_all(theirs.path().join("albums"))
            .expect("a temporary folder is writable");
        for (path, content) in [
            ("albums/spring.jpg", &b"spring"[..]),
            ("albums/summer.jpg", &b"summer"[..]),
        ] {
            std::fs::write(theirs.path().join(path), content)
                .expect("a temporary file is writable");
        }
        let filled = InMemoryIndex::new();
        filled
            .set_mapping(Mapping {
                prefix: None,
                local_root: theirs.path().to_path_buf(),
                root_identity: None,
            })
            .await
            .expect("a mapping is recorded");
        let committed = sync_folders(SyncRequest::new(
            store.as_ref(),
            &filled,
            &keys(),
            theirs.path().join("spool"),
            BatchId::new("run-1"),
            DeviceTime::from_unix_seconds(1_700_000_000),
        ))
        .await
        .expect("the folder is carried into the Library");
        assert_eq!(committed.added.len(), 2);

        // The device that has just joined: the same Library, a folder of its own,
        // and a catalog standing at nothing.
        let joined = InMemoryIndex::new();
        joined
            .set_mapping(Mapping {
                prefix: None,
                local_root: mine.path().to_path_buf(),
                root_identity: None,
            })
            .await
            .expect("a mapping is recorded");
        let library = device(&store, Arc::new(joined));
        assert!(
            library
                .index
                .checkpoint()
                .await
                .expect("the catalog answers")
                .is_none(),
            "a device that has just joined stands at no committed state",
        );

        let outcome = library.catch_up().await.expect("the head is readable");

        assert!(
            outcome.advanced(),
            "the catalog moved to a head: {outcome:?}"
        );
        assert_eq!(outcome.from, None);
        assert_eq!(outcome.entries_before, 0);
        assert_eq!(outcome.entries_after, 2);
        assert_eq!(outcome.gained(), 2);
        assert_eq!(
            library
                .index
                .checkpoint()
                .await
                .expect("the catalog answers")
                .map(|at| at.head_generation),
            outcome.to,
            "what the outcome says is where the catalog now stands",
        );
        assert_eq!(
            library
                .index
                .entries_under(None)
                .await
                .expect("the catalog answers")
                .len(),
            2,
            "both Entries the other device committed are in this catalog",
        );

        // And nothing on this device's disk. The mapped folder is untouched and
        // the spool was never made: a catch-up fetches nothing and uploads
        // nothing.
        assert_eq!(
            std::fs::read_dir(mine.path())
                .expect("the mapped folder is there")
                .count(),
            0,
            "a catch-up places no file in the folder this device maps",
        );

        // Run again with nothing committed in between: the same head, and a
        // caller that can say so.
        let again = library.catch_up().await.expect("the head is readable");
        assert!(!again.advanced(), "there was nothing new: {again:?}");
        assert_eq!(again.gained(), 0);
        assert_eq!(again.to, outcome.to);
    }
}
