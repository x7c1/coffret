use crate::byte_stream::ByteStream;
use crate::conformance::store_under_test::StoreUnderTest;
use crate::error::Error;

/// The name every case here reserves its slot under.
///
/// A head-chain name, because that is what a commit slot is for; the store sees
/// it as a string like any other.
const SUCCESSOR: &str = "head-1.cfrt";

/// A reserved slot can be spent once.
pub async fn put_if_absent_takes_a_free_slot(fixture: &StoreUnderTest) {
    let store = fixture.store();
    let content = b"the first Journal record".to_vec();

    let slot = store
        .reserve_create(SUCCESSOR)
        .await
        .expect("reserving a commit slot must succeed");

    let object = store
        .put_if_absent(&slot, ByteStream::from(content.clone()))
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
        .reserve_create(SUCCESSOR)
        .await
        .expect("reserving a commit slot must succeed");

    let object = store
        .put_if_absent(&slot, ByteStream::from(winner.clone()))
        .await
        .expect("a free slot must accept its object");

    let error = store
        .put_if_absent(&slot, ByteStream::from(b"the record that lost".to_vec()))
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

/// Two writers handed one reservation still leave one winner.
///
/// The case above spends the slot twice in turn, which proves the condition is
/// evaluated at all; this proves it is evaluated where it matters, with both
/// creates in flight at once. That is the situation a commit actually faces,
/// and it is the one a provider can get wrong while still refusing an obvious
/// duplicate.
///
/// One reservation is made and handed to both writers, because that is what a
/// control head does: it carries a single slot, and every writer that starts
/// from that head spends that one (spec: CP-2). What a store sees of the two
/// writers is bytes and a slot, never which kind of control object each is
/// writing, so nothing here can show that a Journal record and an epoch
/// activation contend for the same slot — that is derived above the port, and
/// `commit_slot_is_kind_independent` in this crate is where it is proved.
pub async fn put_if_absent_settles_a_race_between_two_writers(fixture: &StoreUnderTest) {
    let store = fixture.store();

    let slot = store
        .reserve_create(SUCCESSOR)
        .await
        .expect("reserving a commit slot must succeed");

    let (first, second) = tokio::join!(
        store.put_if_absent(&slot, ByteStream::from(b"one writer's record".to_vec())),
        store.put_if_absent(
            &slot,
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

    // The loser reaches the winner's object through the slot it lost, without
    // looking anything up by name (spec: CP-4, CK-11).
    let object = store
        .object_at(&slot)
        .expect("a spent slot must name the object it holds");
    store
        .get(&object, None)
        .await
        .expect("the object the slot holds must be readable back");
}
