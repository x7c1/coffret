//! A catalog that could not be used, met through every door onto the fetch's
//! vocabulary.
//!
//! The vocabulary names that failure as a variant of its own, and every
//! conversion out of it in this crate takes it back out and reports it as
//! [`Error::Index`] — the name the map flow reports it under too, and the
//! cause a create or join carries it as.
//! What these cases pin is that each door does: the gestures that fetch nothing
//! build their own variants by hand rather than through `?`, so each is a
//! construction point of its own and each could carry the catalog's failure
//! inside a sentence about a path, a file or a folder if it stopped going
//! through the lift.

use std::sync::Arc;

use coffret_model::{LibraryId, MasterKey, MasterKeyEpoch};
use coffret_usecase::{InMemoryStore, IndexError, LibraryKeys, RefusingIndex, UNWATCHED};

use crate::error::Error;
use crate::open_library::OpenLibrary;
use crate::testing::{entry_path, local_fs};

/// A device whose Storage holds nothing and whose catalog cannot say what it
/// maps.
///
/// An empty Library, so a fetch's catch-up has nothing to replay and goes
/// through; what each case meets is the catalog, asked the one question every
/// one of these doors asks.
fn device() -> OpenLibrary {
    OpenLibrary {
        store: Arc::new(InMemoryStore::new(64)),
        index: Arc::new(RefusingIndex::new()),
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

/// Whether `result` is the catalog refusing, and nothing wrapped around it.
fn is_the_catalog<T>(result: &crate::Result<T>) -> bool {
    matches!(
        result,
        Err(Error::Index {
            cause: IndexError::Backend { .. },
        }),
    )
}

#[tokio::test]
async fn asking_where_a_file_belongs_reports_the_catalog_as_the_catalog() {
    let result = device()
        .local_path_of(&entry_path("albums/spring.jpg"))
        .await;
    assert!(is_the_catalog(&result), "got {result:?}");
}

#[tokio::test]
async fn opening_a_placed_file_reports_the_catalog_as_the_catalog() {
    let result = device()
        .open_local_file(&entry_path("albums/spring.jpg"))
        .await
        .map(|_| ());
    assert!(is_the_catalog(&result), "got {result:?}");
}

#[tokio::test]
async fn taking_a_file_in_reports_the_catalog_as_the_catalog() {
    let result = device()
        .receive_file(&entry_path("albums/spring.jpg"))
        .await
        .map(|_| ());
    assert!(is_the_catalog(&result), "got {result:?}");
}

#[tokio::test]
async fn reading_a_mapped_folder_reports_the_catalog_as_the_catalog() {
    let library = device();

    let listed = library.added_locally(Some(&entry_path("albums"))).await;
    assert!(is_the_catalog(&listed), "got {listed:?}");

    let one = library
        .added_at(&entry_path("albums/spring.jpg"))
        .await
        .map(|_| ());
    assert!(is_the_catalog(&one), "got {one:?}");
}

#[tokio::test]
async fn a_fetch_reports_the_catalog_as_the_catalog() {
    let result = device().fetch(None, &UNWATCHED).await;
    assert!(is_the_catalog(&result), "got {result:?}");
}
