use crate::byte_stream::ByteStream;
use crate::conformance::listing_walk::ListingWalk;
use crate::conformance::store_under_test::StoreUnderTest;
use crate::error::Error;
use crate::object_ref::ObjectRef;
use crate::object_store::ObjectStore;

/// Trashing takes an object out of the listing and leaves its neighbours alone.
pub async fn trash_hides_an_object_from_list(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let removed = put_object(store, "head-1.cfrt").await;
    put_object(store, "head-2.cfrt").await;

    store
        .trash(&removed)
        .await
        .expect("trashing an object must succeed");

    let walk = ListingWalk::read(store).await;
    assert_eq!(walk.distinct_names(), ["head-2.cfrt"]);
}

/// Purging a live object leaves nothing behind.
pub async fn purge_removes_a_live_object(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let object = put_object(store, "head-1.cfrt").await;

    store
        .purge(&object)
        .await
        .expect("purging a live object must succeed");

    assert_gone(store, &object).await;
}

/// Purging reaches an object that was trashed first.
///
/// Rotation purges whatever old-epoch control objects it finds, and finding one
/// already trashed must not leave a copy a leaked old Recovery Code could still
/// open (spec: MR-3).
pub async fn purge_removes_a_trashed_object(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let object = put_object(store, "head-1.cfrt").await;

    store
        .trash(&object)
        .await
        .expect("trashing an object must succeed");

    store
        .purge(&object)
        .await
        .expect("purging a trashed object must succeed");

    assert_gone(store, &object).await;
}

/// Purging something already gone succeeds.
///
/// A rotation that was interrupted is simply run again, so every purge it
/// repeats has to be a no-op rather than an error that stalls the retry.
pub async fn purge_is_idempotent(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let object = put_object(store, "head-1.cfrt").await;

    store
        .purge(&object)
        .await
        .expect("purging a live object must succeed");

    store
        .purge(&object)
        .await
        .expect("purging an object already gone must succeed");

    store
        .purge(&ObjectRef::new("head-404.cfrt"))
        .await
        .expect("purging an object that never existed must succeed");
}

/// Stores an object whose bytes are its own name, and hands back its reference.
async fn put_object(store: &dyn ObjectStore, name: &str) -> ObjectRef {
    store
        .put(name, ByteStream::from(name.as_bytes()))
        .await
        .expect("putting an object must succeed")
}

/// Asserts an object is gone from both the listing and a direct read.
async fn assert_gone(store: &dyn ObjectStore, object: &ObjectRef) {
    let walk = ListingWalk::read(store).await;
    assert!(
        walk.names().is_empty(),
        "a purged object is still listed: {:?}",
        walk.names()
    );

    let error = store
        .get(object, None)
        .await
        .expect_err("a purged object must not be readable");

    assert!(
        matches!(error, Error::NotFound { .. }),
        "expected a not-found error, got {error:?}"
    );
}
