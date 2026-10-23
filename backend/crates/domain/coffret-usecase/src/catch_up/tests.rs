//! What a catch-up and a look at the remote head answer when keeping the
//! listing they walked fails.
//!
//! Keeping it is a cache for the next Entry fetch, and nothing either of them
//! answers depends on it: a catalog that will not say which Containers are
//! current is logged, and the answer already in hand stands.

use coffret_logging::testing::CapturedLogs;
use coffret_model::{MasterKey, MasterKeyEpoch};
use tracing::Level;

use super::{catch_up_catalog, remote_head, CatchUpRequest, RemoteHead};
use crate::commit::{write_first_head, CommitPolicy};
use crate::device_state::DeviceTime;
use crate::fetch::FetchMemory;
use crate::freeze::LibraryKeys;
use crate::in_memory_fs::InMemoryFs;
use crate::in_memory_store::InMemoryStore;
use crate::index::Index;
use crate::refusing_index::RefusingIndex;

fn keys() -> LibraryKeys {
    LibraryKeys::derive(
        &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
        MasterKeyEpoch::FIRST,
    )
}

/// A store holding a Library as creating one leaves it: its first head.
async fn created(keys: &LibraryKeys) -> InMemoryStore {
    let store = InMemoryStore::new(8);
    write_first_head(&store, keys.control(), &CommitPolicy::default())
        .await
        .expect("creating a Library in an empty store must succeed")
        .expect("an empty store holds no head to find instead");
    store
}

/// Asserts that the one warning is the listing not being kept, with the
/// catalog's refusal as its reason.
fn warned_of_the_listing(logs: &CapturedLogs) {
    let warned = logs.only(Level::WARN);
    assert!(warned.message().contains("listing is not kept"), "{warned}",);
    assert!(
        warned
            .field("reason")
            .contains("reading the current Containers"),
        "{warned}",
    );
}

// The catalog has advanced to the head whether or not the walk is kept, so the
// catch-up answers with the head it reached.
#[tokio::test]
async fn a_catch_up_whose_listing_is_not_kept_still_answers() {
    let keys = keys();
    let store = created(&keys).await;
    let index = RefusingIndex::refusing_containers();
    let fs = InMemoryFs::new();
    let memory = FetchMemory::default();
    let logs = CapturedLogs::capture();

    let caught = catch_up_catalog(
        CatchUpRequest::new(
            &store,
            &index,
            &keys,
            &fs,
            DeviceTime::from_unix_seconds(1_700_000_000),
        )
        .remembering(&memory),
    )
    .await
    .unwrap_or_else(|error| panic!("the catch-up must answer: {error}"));

    assert_eq!(caught.from, None);
    assert!(caught.to.is_some(), "the catalog reached the first head");
    warned_of_the_listing(&logs);
}

// The comparison is made before anything is kept, so a catalog that will not
// say which Containers are current leaves `Current` standing.
#[tokio::test]
async fn a_current_head_whose_listing_is_not_kept_is_still_current() {
    let keys = keys();
    let store = created(&keys).await;
    let index = RefusingIndex::refusing_containers();
    let fs = InMemoryFs::new();
    catch_up_catalog(CatchUpRequest::new(
        &store,
        &index,
        &keys,
        &fs,
        DeviceTime::from_unix_seconds(1_700_000_000),
    ))
    .await
    .unwrap_or_else(|error| panic!("the catch-up must answer: {error}"));
    assert!(index.checkpoint().await.expect("a checkpoint").is_some());

    let memory = FetchMemory::default();
    let logs = CapturedLogs::capture();
    let head = remote_head(
        &store,
        &index,
        &CommitPolicy::default().retry,
        Some(&memory),
    )
    .await
    .unwrap_or_else(|error| panic!("the look at the head must answer: {error}"));

    assert_eq!(head, RemoteHead::Current);
    warned_of_the_listing(&logs);
}
