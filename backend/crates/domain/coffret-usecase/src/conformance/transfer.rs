use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::conformance::store_under_test::StoreUnderTest;
use crate::error::Error;

/// What goes in comes out, byte for byte.
pub async fn put_get_round_trips_content(fixture: &StoreUnderTest) {
    let store = fixture.store();
    let content = b"a Storage Object's ciphertext".to_vec();

    let object = store
        .put("head-1.cfrt", ByteStream::from(content.clone()))
        .await
        .expect("putting an object must succeed");

    let stored = store
        .get(&object, None)
        .await
        .expect("getting an object just put must succeed");

    assert_eq!(stored.len(), content.len() as u64);
    assert_eq!(stored.into_bytes().await.unwrap(), content);
}

/// A zero-length object is an object, not an absence.
///
/// Providers like to treat "no bytes" as "no object"; the format layer does
/// not, and an adapter that loses the difference would report a stored object
/// as missing.
pub async fn put_get_round_trips_a_zero_length_object(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let object = store
        .put("head-2.cfrt", ByteStream::from(Vec::new()))
        .await
        .expect("putting a zero-length object must succeed");

    let stored = store
        .get(&object, None)
        .await
        .expect("a zero-length object must be readable back");

    assert_eq!(stored.len(), 0);
    assert_eq!(stored.into_bytes().await.unwrap(), Vec::<u8>::new());
}

/// A ranged read serves exactly the half-open range asked for.
///
/// Reading one chunk out of a Container depends on it: the whole point is not
/// to download the object to reach its middle.
pub async fn get_reads_a_byte_range(fixture: &StoreUnderTest) {
    let store = fixture.store();
    let content: Vec<u8> = (0..=255u8).collect();

    let object = store
        .put("head-3.cfrt", ByteStream::from(content.clone()))
        .await
        .expect("putting an object must succeed");

    let stored = store
        .get(&object, Some(10..20))
        .await
        .expect("a ranged read must succeed");

    assert_eq!(stored.len(), 10);
    assert_eq!(stored.into_bytes().await.unwrap(), content[10..20]);
}

/// A missing object is reported as missing, and not worth asking again for.
pub async fn get_reports_a_missing_object(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let error = store
        .get(&ObjectRef::new("head-404.cfrt"), None)
        .await
        .expect_err("getting an object that was never stored must fail");

    assert!(
        matches!(error, Error::NotFound { .. }),
        "expected a not-found error, got {error:?}"
    );
    assert!(!error.is_retryable());
}
