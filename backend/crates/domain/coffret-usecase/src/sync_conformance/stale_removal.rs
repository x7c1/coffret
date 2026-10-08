use crate::commit::CommitError;
use crate::conformance_library::Library;
use crate::entry_paths::entry_path;
use crate::removing_rival::RemovingRival;
use crate::sync::{sync_folders, Disposal, Settled, SyncError};
use crate::sync_conformance::fixtures::{
    keys, map, pending, policy, request, touch, write, NEWER, OLDER,
};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A replacement for a one-file Container another device removed while this run
/// was preparing it is refused, and the removal stands (spec: CP-7, CP-18).
///
/// The run read the head, found the file modified, and uploaded a replacement
/// for the Container that held it. Before its commit caught up again, another
/// device removed that Container. Committing the replacement anyway would land
/// it as if the removal had never happened, so the commit refuses the batch as
/// a conflict naming the Container, the head stays the other device's, and the
/// run ends with the refusal rather than retrying the same batch.
///
/// The batch was refused before any commit was attempted, so the next run
/// settles the replacement's upload like any other upload whose batch never
/// committed: its object goes to the trash and its row goes (spec: OC-2, OC-3).
pub async fn a_replacement_for_a_removed_container_is_refused(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let path = write(
        fixture.fs(),
        fixture.folder(),
        "a.jpg",
        b"the original bytes",
    );
    touch(fixture.fs(), &path, OLDER);
    let first = sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .expect("a first sync must succeed");
    let original = *first
        .added
        .first()
        .expect("the first sync uploaded the file");

    fixture
        .fs()
        .write_file(&path, b"bytes that are not the original ones");
    touch(fixture.fs(), &path, NEWER);

    let rival = RemovingRival::removing(store, keys.control(), policy(), vec![original]);
    let result = sync_folders(request(&rival, index, &keys, fixture.fs(), 2)).await;
    assert!(rival.has_removed(), "the other device removed it mid-run");

    match result {
        Err(SyncError::Commit(failure)) => match *failure.error {
            CommitError::RemovalNotCurrent { ref container_ids } => {
                assert_eq!(container_ids, &vec![original]);
            }
            ref other => panic!("expected the stale removal to be refused, got {other:?}"),
        },
        other => panic!("expected the commit to refuse the batch, got {other:?}"),
    }

    let library = Library::read(store).await;
    assert_eq!(
        library.heads(),
        2,
        "the first sync's head and the other device's, and none of this run's",
    );
    assert!(
        index
            .entry_at(&entry_path("a.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_none(),
        "the removed Entry stays removed",
    );

    let rows = pending(index).await;
    let [row] = &rows[..] else {
        panic!("the refused replacement leaves its one row, got {rows:?}");
    };
    assert!(
        !row.commit_attempted,
        "the refusal came before any commit was attempted",
    );
    let refused = row.container_id;
    assert!(library.holds_container(refused));

    // The next run settles what the refused one uploaded, then plans afresh
    // from the state the other device left.
    let next = sync_folders(request(store, index, &keys, fixture.fs(), 3))
        .await
        .expect("the next run plans from the new state");
    assert!(
        matches!(
            &next.settled[..],
            [Settled::Disposed { container_id, disposal: Disposal::Trashed }]
                if *container_id == refused
        ),
        "the refused upload is disposed of, got {:?}",
        next.settled,
    );
    assert!(
        next.replaced.is_empty(),
        "nothing replaces a Container that is no longer current",
    );
    assert!(!Library::read(store).await.holds_container(refused));
    assert!(pending(index).await.is_empty());
}
