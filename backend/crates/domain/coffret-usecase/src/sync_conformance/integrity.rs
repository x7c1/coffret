use coffret_model::EntryPath;

use crate::sync::{sync_folders, SyncError};
use crate::sync_conformance::fixtures::{keys, map, request, spooled, write};
use crate::sync_conformance::mangling_store::ManglingStore;
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// An object Storage says is not the bytes that were sent stops the run.
///
/// The comparison is provider-scoped and that is all it claims to be: it asks
/// one provider whether what it stored is what left this device. A disagreement
/// means the object is not the Container the batch would name, so the run stops
/// with a verdict of its own rather than committing and hoping — and because it
/// stops before the Journal record, the Library is exactly where it was
/// (spec: CP-1).
///
/// What is left behind is not lost: the spool and its pending row still name
/// what this device created, which is what lets a later run dispose of it
/// (spec: OC-2).
pub async fn a_provider_hash_mismatch_is_refused(fixture: &SyncUnderTest) {
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    write(fixture.folder(), "a.jpg", b"the file's bytes").await;
    let mangling = ManglingStore::around(fixture.store());

    let result = sync_folders(request(&mangling, index, &keys, fixture.spool(), 1)).await;

    let Err(SyncError::TransferCorrupted {
        container_id,
        expected,
        actual,
    }) = result
    else {
        panic!("expected a transfer the provider disagrees about to be refused, got {result:?}");
    };
    assert_ne!(expected, actual);

    assert!(
        index
            .entry_at(&EntryPath::new("a.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_none(),
        "nothing was committed, so nothing is current (spec: CP-1)",
    );
    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "the Library stands where it stood",
    );

    let pending = index
        .pending_uploads()
        .await
        .expect("asking the Index for pending uploads must succeed");
    assert_eq!(
        pending.len(),
        1,
        "the Container this run created is still accounted for (spec: OC-2)",
    );
    assert_eq!(pending[0].container_id, container_id);
    assert_eq!(
        spooled(fixture.spool()).await,
        1,
        "its ciphertext is still where the row says it is",
    );

    // And a later run, against a store that answers honestly, converges: the
    // abandoned Container goes and the file is committed once.
    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.spool(), 2))
        .await
        .expect("a run against an honest store must succeed");
    assert_eq!(outcome.added.len(), 1);
    assert_ne!(outcome.added[0], container_id);
    assert_eq!(outcome.reconciled.len(), 1);
    assert_eq!(outcome.reconciled[0].container_id(), container_id);
    assert_eq!(spooled(fixture.spool()).await, 0);
}
