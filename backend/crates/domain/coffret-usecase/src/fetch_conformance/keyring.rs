use crate::entry_paths::entry_path;
use crate::fetch::{fetch_folders, Surfaced};
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    entry_at, exists, keys, lose_key, map, overwrite, read, replica_name, request, sync_source,
    write,
};

/// A Container the committed Keyring has no key for is reported locked, and the
/// rest of the batch is fetched.
///
/// A key-lost marker is a statement about the committed control state and nothing
/// else: the Container stays current, its ciphertext stays where it is, and it
/// leaves the current set only through a genuine committed removal (spec: KL-7,
/// KL-17). So the loss costs exactly the Entries that Container holds — they are
/// reported locked rather than fetched (spec: RV-2, RV-7) — and everything else in
/// the run is placed as usual.
///
/// A fetch that failed the whole run here would make one lost key look like a lost
/// Library.
pub async fn a_key_lost_container_is_locked_and_the_rest_is_fetched(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    let readable = b"the file whose key survived".as_slice();
    write(fixture.source_folder(), "a.jpg", readable).await;
    write(
        fixture.source_folder(),
        "b.jpg",
        b"the file whose key is gone",
    )
    .await;
    sync_source(fixture, &keys, 1).await;

    let locked = entry_at(fixture.source(), "b.jpg").await.container_id;
    lose_key(fixture.store(), fixture.source(), locked).await;

    let outcome = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2))
        .await
        .unwrap_or_else(|error| panic!("a fetch meeting a lost key must succeed: {error}"));

    assert_eq!(
        outcome.fetched,
        vec![entry_path("a.jpg")],
        "the Container whose key survived was fetched and placed",
    );
    assert_eq!(outcome.locked, vec![locked]);
    assert_eq!(
        outcome.surfaced,
        vec![Surfaced::KeyLost {
            path: entry_path("b.jpg"),
            container_id: locked,
        }],
    );

    assert_eq!(read(&fixture.target_folder().join("a.jpg")).await, readable);
    assert!(
        !exists(&fixture.target_folder().join("b.jpg")).await,
        "a locked Container places nothing",
    );
    assert_eq!(
        entry_at(fixture.target(), "b.jpg").await.container_id,
        locked,
        "and stays current all the same (spec: KL-17)",
    );
}

/// A first Keyring replica that will not open costs a read and nothing else.
///
/// One committed valid replica carries the whole logical Keyring, so the replica
/// count is redundancy and never a quorum (spec: KL-6): the walk steps over the
/// position it cannot read and takes the next. That is what makes a degraded set
/// still serve a restore rather than gate one (spec: KL-5, RV-2) — repairing it is
/// a separate obligation (spec: KL-13) and no part of a fetch.
pub async fn a_mangled_first_keyring_replica_falls_back(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    let content = b"the file behind a degraded Keyring".as_slice();
    write(fixture.source_folder(), "a.jpg", content).await;
    sync_source(fixture, &keys, 1).await;

    let committed = fixture
        .source()
        .checkpoint()
        .await
        .expect("reading the source checkpoint must succeed")
        .expect("the source device committed")
        .keyring;
    assert!(
        committed.replica_count() >= 2,
        "the case needs a second position to fall back onto (spec: KL-8)",
    );
    overwrite(
        fixture.store(),
        &replica_name(&committed, 0),
        b"not a Keyring replica at all".to_vec(),
    )
    .await;

    let outcome = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2))
        .await
        .unwrap_or_else(|error| {
            panic!("a fetch against a degraded Keyring set must succeed: {error}")
        });

    assert_eq!(outcome.fetched, vec![entry_path("a.jpg")]);
    assert!(outcome.locked.is_empty(), "no key was lost, only a replica");
    assert!(outcome.surfaced.is_empty());
    assert_eq!(read(&fixture.target_folder().join("a.jpg")).await, content);
}
