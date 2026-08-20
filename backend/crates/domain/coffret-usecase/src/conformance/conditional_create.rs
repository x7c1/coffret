use crate::byte_stream::ByteStream;
use crate::conformance::store_under_test::StoreUnderTest;
use crate::error::Error;

/// A reserved slot can be spent once.
pub async fn put_if_absent_takes_a_free_slot(fixture: &StoreUnderTest) {
    let store = fixture.store();
    let content = b"the first Journal record".to_vec();

    let slot = store
        .reserve_create()
        .await
        .expect("reserving a commit slot must succeed");

    let object = store
        .put_if_absent(&slot, "jrn-1.cfrt", ByteStream::from(content.clone()))
        .await
        .expect("a free slot must accept its object");

    let stored = store
        .get(&object, None)
        .await
        .expect("a conditionally created object must be readable back");

    assert_eq!(stored.into_bytes().await.unwrap(), content);
}

/// Spending the same slot twice loses the race, and changes nothing.
///
/// This is the commit primitive: of the writers that start from one control
/// head exactly one commits (spec: CP-3), and the losers must be able to tell
/// that they lost — rather than that the network failed — so they can refresh
/// the head, reconcile, and retry instead of overwriting the winner
/// (spec: CP-4).
pub async fn put_if_absent_rejects_a_taken_slot(fixture: &StoreUnderTest) {
    let store = fixture.store();
    let winner = b"the record that committed".to_vec();

    let slot = store
        .reserve_create()
        .await
        .expect("reserving a commit slot must succeed");

    let object = store
        .put_if_absent(&slot, "jrn-1.cfrt", ByteStream::from(winner.clone()))
        .await
        .expect("a free slot must accept its object");

    let error = store
        .put_if_absent(
            &slot,
            "jrn-1.cfrt",
            ByteStream::from(b"the record that lost".to_vec()),
        )
        .await
        .expect_err("a slot already spent must not accept a second object");

    assert!(
        matches!(error, Error::AlreadyExists { .. }),
        "expected an already-exists error, got {error:?}"
    );
    assert!(!error.is_retryable());

    let stored = store
        .get(&object, None)
        .await
        .expect("the winner's object must survive the loser's attempt");

    assert_eq!(stored.into_bytes().await.unwrap(), winner);
}

/// Two writers spending one slot at the same moment still leave one winner.
///
/// The case above spends the slot twice in turn, which proves the condition is
/// evaluated at all; this proves it is evaluated where it matters, with both
/// creates in flight at once. That is the situation a commit actually faces,
/// and it is the one a provider can get wrong while still refusing an obvious
/// duplicate.
pub async fn put_if_absent_settles_a_race_between_two_writers(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let slot = store
        .reserve_create()
        .await
        .expect("reserving a commit slot must succeed");

    let (first, second) = tokio::join!(
        store.put_if_absent(
            &slot,
            "jrn-1.cfrt",
            ByteStream::from(b"one writer's record".to_vec())
        ),
        store.put_if_absent(
            &slot,
            "jrn-1.cfrt",
            ByteStream::from(b"the other writer's record".to_vec())
        ),
    );

    let outcomes = [&first, &second];
    assert_eq!(
        outcomes
            .into_iter()
            .filter(|outcome| outcome.is_ok())
            .count(),
        1,
        "expected exactly one winner, got {first:?} and {second:?}"
    );
    assert_eq!(
        outcomes
            .into_iter()
            .filter(|outcome| matches!(outcome, Err(Error::AlreadyExists { .. })))
            .count(),
        1,
        "expected exactly one lost race, got {first:?} and {second:?}"
    );
}
