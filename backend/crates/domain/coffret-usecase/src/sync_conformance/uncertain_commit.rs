use super::fixtures::{keys, map, pending, request, spooled, write};
use super::interruption::interrupted;
use super::sync_under_test::SyncUnderTest;
use crate::conformance_library::Library;
use crate::index_error::IndexError;
use crate::sync::{sync_folders, Settled, SyncError};

/// An attempted commit with no reachable record remains ambiguous (spec: OC-3).
/// Repeated cleanup preserves its object, spool, and durable evidence.
pub async fn an_uncertain_commit_retains_its_ciphertext_and_provenance(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;
    let uncertain = interrupted(fixture, index, Some(store)).await;
    let mut row = pending(index).await.remove(0);
    row.commit_attempted = true;
    index
        .record_pending_row(row.clone())
        .await
        .expect("record the attempt");
    for time in [2, 3] {
        let outcome = sync_folders(request(store, index, &keys, fixture.fs(), time))
            .await
            .expect("an uncertain attempt is retained");
        assert!(
            matches!(&outcome.settled[..], [Settled::Retained { container_id }] if *container_id == uncertain)
        );
        assert_eq!(pending(index).await, vec![row.clone()]);
        assert_eq!(spooled(fixture.fs()), 1);
        assert!(Library::read(store).await.holds_container(uncertain));
    }
}

/// A concurrent run cannot settle a producer's in-flight rows (spec: OC-2).
pub async fn a_live_producer_keeps_exclusive_ownership(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    let owner = index
        .own_pending_rows()
        .await
        .expect("the producer owns its rows");
    let active = interrupted(fixture, index, Some(store)).await;
    let result = sync_folders(request(store, index, &keys, fixture.fs(), 2)).await;
    assert!(matches!(
        result,
        Err(SyncError::Index(IndexError::PendingRowsBusy { .. }))
    ));
    assert!(Library::read(store).await.holds_container(active));
    assert_eq!(pending(index).await.len(), 1);
    assert_eq!(spooled(fixture.fs()), 1);
    drop(owner);
    let _next = index
        .own_pending_rows()
        .await
        .expect("ownership is released");
}

/// A committed Container survives a lost response followed by withheld history.
/// Once history is visible again, the interrupted refresh completes (spec: OC-7).
pub async fn a_lost_response_and_withheld_head_never_authorize_trash(fixture: &SyncUnderTest) {
    use super::withheld_commit::WithheldCommit;
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;
    write(fixture.fs(), fixture.folder(), "a.jpg", b"committed bytes");
    let withheld = WithheldCommit { inner: store };
    let failed = sync_folders(request(&withheld, index, &keys, fixture.fs(), 1)).await;
    assert!(failed.is_err(), "the commit response is unavailable");
    let rows = pending(index).await;
    assert_eq!(rows.len(), 1);
    assert!(rows[0].commit_attempted, "the marker precedes network IO");
    let committed = rows[0].container_id;
    fixture.fs().remove_file(&fixture.folder().join("a.jpg"));
    let outcome = sync_folders(request(&withheld, index, &keys, fixture.fs(), 2))
        .await
        .expect("the stale listing cannot authorize deletion");
    assert!(
        matches!(&outcome.settled[..], [Settled::Retained { container_id }] if *container_id == committed)
    );
    assert!(Library::read(store).await.holds_container(committed));
    assert_eq!(pending(index).await, rows);
    assert_eq!(spooled(fixture.fs()), 1);
    write(fixture.fs(), fixture.folder(), "a.jpg", b"committed bytes");
    let recovered = sync_folders(request(store, index, &keys, fixture.fs(), 3))
        .await
        .expect("visible history completes the interrupted refresh");
    assert!(
        matches!(&recovered.settled[..], [Settled::Completed { container_id, .. }] if *container_id == committed)
    );
    assert!(pending(index).await.is_empty());
    assert_eq!(spooled(fixture.fs()), 0);
    assert!(Library::read(store).await.holds_container(committed));
}

/// A different committed writer does not prove which slot this row attempted.
/// Without that durable association, keep the candidate (spec: OC-3).
pub async fn another_writers_head_does_not_prove_an_uncertain_attempt(fixture: &SyncUnderTest) {
    use crate::{InMemoryIndex, Index};
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;
    let uncertain = interrupted(fixture, index, Some(store)).await;
    let mut row = pending(index).await.remove(0);
    row.commit_attempted = true;
    index.record_pending_row(row.clone()).await.unwrap();

    let rival = InMemoryIndex::new();
    for mapping in index.mappings().await.unwrap() {
        rival.set_mapping(mapping).await.unwrap();
    }
    write(
        fixture.fs(),
        fixture.folder(),
        "rival.jpg",
        b"another writer",
    );
    let committed = sync_folders(request(store, &rival, &keys, fixture.fs(), 2))
        .await
        .expect("another writer occupies the Library's first slot");
    assert!(committed.commit.is_some());
    fixture
        .fs()
        .remove_file(&fixture.folder().join("rival.jpg"));

    let outcome = sync_folders(request(store, index, &keys, fixture.fs(), 3))
        .await
        .expect("catch-up can authenticate the other writer without disposing of this row");
    assert!(matches!(
        &outcome.settled[..],
        [Settled::Retained { container_id }] if *container_id == uncertain
    ));
    assert_eq!(pending(index).await, vec![row]);
    assert_eq!(spooled(fixture.fs()), 1);
    assert!(Library::read(store).await.holds_container(uncertain));
}
