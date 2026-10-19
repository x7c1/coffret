use coffret_model::Passphrase;
use coffret_usecase::catch_up::{
    catch_up_catalog, remote_head, CatchUpOutcome, CatchUpRequest, RemoteHead,
};
use coffret_usecase::commit::CommitPolicy;
use tracing::info;

use crate::device_time::now;
use crate::error::{Error, Result};
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
    /// Nothing in the mapped folders changes. Every Entry the run learns of is
    /// `remote` until something asks for its bytes (spec: EP-10), which is what
    /// makes this the cheap question — the control objects and nothing else.
    /// The things on this device's disk it may touch are its own: a kept parcel
    /// of a Container the replay took out of the current set, which is let go
    /// (spec: PK-21), and the spools of a batch of this device's whose record
    /// the replay found committed, whose bookkeeping is completed — the files it
    /// carried in recorded as this device's, `present` rather than `remote` —
    /// unless a sync, a freeze or a deletion running beside this owns them
    /// (spec: OC-2, OC-7).
    pub async fn catch_up(&self) -> Result<CatchUpOutcome> {
        info!(
            operation = "catch_up",
            library = %self.library_id,
            "catching the catalog up with the Library's head"
        );
        // The listing the replay walks is the one the fetches after it reach
        // objects through, so it is left where they look for it.
        let outcome = catch_up_catalog(
            CatchUpRequest::new(
                self.store.as_ref(),
                self.index.as_ref(),
                &self.keys,
                self.local_fs.as_ref(),
                now(),
            )
            .remembering(&self.fetch_memory),
        )
        .await?;
        // A Container the replay took out of the current set takes any parcel
        // this device kept of it along (spec: PK-21).
        self.let_go_parcels().await;
        Ok(outcome)
    }

    /// Whether the Library has a head this device's catalog has not caught up
    /// with, asked of the listing's names alone.
    ///
    /// The question before [`catch_up`](Self::catch_up), for a caller that
    /// wants to say there is something new without bringing it in: no object is
    /// read, so neither the Keyring nor any key is reached for, and the catalog
    /// is left where it stands (spec: CK-9, FM-12). A listing that names the
    /// catalog's own head is kept for the Entry fetches after it, as the one a
    /// catch-up walks is.
    ///
    /// Not logged at `info`, unlike the catch-up: a process asks this on a
    /// timer, and a line for every answer would bury the runs that changed
    /// something.
    pub async fn remote_head(&self) -> Result<RemoteHead> {
        remote_head(
            self.store.as_ref(),
            self.index.as_ref(),
            &CommitPolicy::default().retry,
            Some(&self.fetch_memory),
        )
        .await
        .map_err(|cause| Error::RemoteHeadNotRead {
            cause: Box::new(cause),
        })
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
    P: FnOnce() -> Result<Passphrase> + Send,
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

    use super::RemoteHead;
    use crate::open_library::OpenLibrary;
    use crate::testing::local_fs;

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
            local_fs: local_fs(),
            keys: keys(),
            spool: std::env::temp_dir().join("coffret-a-catch-up-never-spools"),
            parcel_dir: std::env::temp_dir().join("coffret-a-catch-up-keeps-no-parcel"),
            parcel_len: coffret_format::PARCEL_LEN,
            library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
            epoch: MasterKeyEpoch::FIRST,
            provider: "s3",
            grant: None,
            births: Default::default(),
            fetch_memory: Default::default(),
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
            .set_mapping(Mapping::new(None, theirs.path().to_path_buf()))
            .await
            .expect("a mapping is recorded");
        let committed = sync_folders(SyncRequest::new(
            store.as_ref(),
            &filled,
            &keys(),
            local_fs().as_ref(),
            local_fs().as_ref(),
            local_fs().as_ref(),
            local_fs().as_ref(),
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
            .set_mapping(Mapping::new(None, mine.path().to_path_buf()))
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
                .map(|at| at.head_generation()),
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

    // The look before a catch-up: a device that has replayed nothing sees a
    // newer head in a Library that has committed, and saying so moves nothing —
    // the catalog still stands at no committed state. Once a catch-up lands it
    // is current.
    #[tokio::test]
    async fn the_remote_head_says_newer_until_a_catch_up_lands() {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new(64));
        let theirs = tempfile::tempdir().expect("a temporary directory must be available");
        std::fs::write(theirs.path().join("spring.jpg"), b"spring")
            .expect("a temporary file is writable");
        let filled = InMemoryIndex::new();
        filled
            .set_mapping(Mapping::new(None, theirs.path().to_path_buf()))
            .await
            .expect("a mapping is recorded");
        sync_folders(SyncRequest::new(
            store.as_ref(),
            &filled,
            &keys(),
            local_fs().as_ref(),
            local_fs().as_ref(),
            local_fs().as_ref(),
            local_fs().as_ref(),
            theirs.path().join("spool"),
            BatchId::new("run-1"),
            DeviceTime::from_unix_seconds(1_700_000_000),
        ))
        .await
        .expect("the folder is carried into the Library");

        let library = device(&store, Arc::new(InMemoryIndex::new()));
        assert_eq!(
            library
                .remote_head()
                .await
                .expect("the listing is readable"),
            RemoteHead::Newer,
        );
        assert!(
            library
                .index
                .checkpoint()
                .await
                .expect("the catalog answers")
                .is_none(),
            "looking at the head moved nothing",
        );

        library.catch_up().await.expect("the head is readable");
        assert_eq!(
            library
                .remote_head()
                .await
                .expect("the listing is readable"),
            RemoteHead::Current,
        );
    }
}
