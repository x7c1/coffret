use crate::byte_stream::ByteStream;
use crate::conformance::listing_walk::ListingWalk;
use crate::conformance::store_under_test::StoreUnderTest;

/// An empty store lists nothing and offers nowhere to continue.
///
/// A first page that carried a token here would send a caller round a loop that
/// never ends.
pub async fn list_is_empty_on_a_fresh_store(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let page = store
        .list(None)
        .await
        .expect("listing an empty store must succeed");

    assert!(page.objects.is_empty(), "found {:?}", page.objects);
    assert_eq!(page.next, None);
}

/// More objects than fit on a page are still walked exactly once.
pub async fn list_walks_every_page_exactly_once(fixture: &StoreUnderTest) {
    let store = fixture.store();

    // One more than two full pages, so the walk has to follow a token and the
    // last page is a partial one.
    let count = fixture.page_size() * 2 + 1;
    let mut expected: Vec<String> = Vec::with_capacity(count);
    for index in 0..count {
        let name = format!("head-{index}.cfrt");
        store
            .put(&name, ByteStream::from(name.clone().into_bytes()))
            .await
            .expect("putting an object must succeed");

        expected.push(name);
    }
    expected.sort();

    let walk = ListingWalk::read(store).await;

    assert_eq!(walk.distinct_names(), expected);
    assert!(
        walk.page_count() > 1,
        "{count} objects came back on a single page of {}",
        fixture.page_size()
    );
}

/// A listed object is reported under the name it was stored as, with the
/// provider's digest of its bytes and a reference that reads them back.
///
/// Scanning Storage is how a Library is rebuilt without an Index, and a scan
/// that could not match a listed object against what was uploaded — or reach
/// the bytes it names — would have to download everything to find out.
pub async fn list_reports_what_it_stored(fixture: &StoreUnderTest) {
    let store = fixture.store();
    let content = b"a Keyring replica".to_vec();

    store
        .put("key-1-ab-r0-of-1.cfrt", ByteStream::from(content.clone()))
        .await
        .expect("putting an object must succeed");

    let page = store.list(None).await.expect("listing must succeed");

    let [object] = page.objects.as_slice() else {
        panic!("expected exactly one object, found {:?}", page.objects);
    };
    assert_eq!(object.name, "key-1-ab-r0-of-1.cfrt");
    assert!(
        object.hash.is_some(),
        "a listing must carry the provider's digest of the stored bytes"
    );

    let stored = store
        .get(&object.object_ref, None)
        .await
        .expect("the reference a listing reports must be readable");

    assert_eq!(stored.into_bytes().await.unwrap(), content);
}
