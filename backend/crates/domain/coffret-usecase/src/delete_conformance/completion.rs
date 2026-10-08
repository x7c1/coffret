use crate::commit::{CommitError, CommitFailure};
use crate::delete::{delete_entries, DeleteError};
use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::delete_conformance::fixtures::{
    current, freeze, keys, map, paths, request, spooled, sync_on, write,
};
use crate::entry_paths::entry_path;
use crate::sync::Settled;
use crate::sync_conformance::refusing_index::RefusingIndex;

/// OC-7, EP-10: a rebuilt Pack whose commit landed and whose refresh did not is
/// completed by the next run without being taken for files this device put on
/// disk.
///
/// The deleting device here never held any of the Library's files. The Pack it
/// rebuilt carries Entries forward off Storage, so completing the interrupted
/// bookkeeping must mark none of them present: a device that recorded a file it
/// never placed would read that file's absence on its next scan as a deletion
/// it witnessed (spec: EP-10). What the completion does do is everything else —
/// the spool and the row go, and the Library is left as the commit made it
/// (spec: OC-2, OC-7).
pub async fn a_rebuild_whose_refresh_failed_is_completed_without_claiming_its_files(
    fixture: &DeleteUnderTest,
) {
    let store = fixture.store();
    let other = fixture.other();
    map(fixture).await;
    write(
        fixture,
        "albums/a.jpg",
        b"kept, never on the deleting device",
    );
    write(fixture, "albums/b.jpg", b"deleted");
    write(fixture, "albums/c.jpg", b"kept as well");
    freeze(fixture, 1).await;

    let keys = keys();
    let refusing = RefusingIndex::around(other);
    let result = delete_entries(request(
        store,
        &refusing,
        fixture,
        &keys,
        paths(&["albums/b.jpg"]),
        2,
    ))
    .await;
    let Err(DeleteError::Commit(CommitFailure { error, .. })) = &result else {
        panic!("a refused refresh must fail the run that committed, got {result:?}");
    };
    assert!(
        matches!(error.as_ref(), CommitError::Index(_)),
        "the record landed and the refresh did not, got {error:?}",
    );
    let rows = other
        .pending_rows()
        .await
        .expect("asking for pending rows must succeed");
    assert_eq!(
        rows.len(),
        1,
        "the rebuilt Pack's row survives (spec: OC-2)"
    );
    assert!(
        !rows[0].materializes,
        "and says its Pack was rebuilt, not built out of local files",
    );
    let rebuilt = rows[0].container_id;

    let outcome = sync_on(other, fixture, 3).await;

    assert!(
        matches!(
            &outcome.settled[..],
            [Settled::Completed { container_id, entries: 0 }] if *container_id == rebuilt
        ),
        "the commit landed, so the row completes, and claims no file (spec: OC-7): {:?}",
        outcome.settled,
    );
    for path in ["albums/a.jpg", "albums/c.jpg"] {
        assert_eq!(
            current(other, path).await.map(|at| at.container_id),
            Some(rebuilt),
            "{path} is current in the replacement",
        );
        assert!(
            other
                .local_entry_at(&entry_path(path))
                .await
                .expect("asking for a local row must succeed")
                .is_none(),
            "and this device records no file for it (spec: EP-10)",
        );
    }
    assert!(current(other, "albums/b.jpg").await.is_none());
    assert!(other
        .pending_rows()
        .await
        .expect("asking for pending rows must succeed")
        .is_empty());
    assert_eq!(spooled(fixture.fs()), 0, "and the spool is gone with it");
}
