use crate::commit::CommitError;
use crate::conformance_library::Library;
use crate::delete::{delete_entries, DeleteError};
use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::delete_conformance::fixtures::{
    current, filler, freeze, keys, map, paths, policy, request, write,
};
use crate::removing_rival::RemovingRival;

/// A Pack rebuilt for a deletion while another device removed the whole Pack is
/// refused, and the other device's deletion stands (spec: CP-7, CP-18).
///
/// The run read the head, planned to delete one Entry of a Pack, and uploaded
/// the rebuilt Pack holding the other two. Before its commit caught up again,
/// another device removed that Pack outright. Committing the rebuild anyway
/// would bring the two kept Entries back into the Library after the other
/// device deleted them, so the commit refuses the batch as a conflict naming
/// the Pack: the head stays the other device's, and the rebuilt Pack is left
/// for the next sync to settle like any upload whose batch never committed
/// (spec: OC-2, OC-3).
pub async fn a_rebuild_of_a_pack_another_device_removed_is_refused(fixture: &DeleteUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture).await;
    let relatives = ["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"];
    for (seed, relative) in relatives.iter().enumerate() {
        write(fixture, relative, &filler(80, 0x60 + seed as u8));
    }
    let pack = freeze(fixture, 1).await;

    let rival = RemovingRival::removing(store, keys.control(), policy(), vec![pack]);
    let result = delete_entries(request(
        &rival,
        index,
        fixture,
        &keys,
        paths(&["albums/b.jpg"]),
        2,
    ))
    .await;
    assert!(rival.has_removed(), "the other device removed it mid-run");

    match result {
        Err(DeleteError::Commit(failure)) => match *failure.error {
            CommitError::RemovalNotCurrent { ref container_ids } => {
                assert_eq!(container_ids, &vec![pack]);
            }
            ref other => panic!("expected the stale removal to be refused, got {other:?}"),
        },
        other => panic!("expected the commit to refuse the batch, got {other:?}"),
    }

    let library = Library::read(store).await;
    assert_eq!(
        library.heads(),
        2,
        "the freeze's head and the other device's, and none of this run's",
    );
    for relative in relatives {
        assert!(
            current(index, relative).await.is_none(),
            "{relative} stays deleted, kept Entries included",
        );
    }

    let rows = index
        .pending_rows()
        .await
        .expect("asking the Index for pending rows must succeed");
    let [row] = &rows[..] else {
        panic!("the rebuilt Pack keeps its one row, got {rows:?}");
    };
    assert!(
        !row.commit_attempted,
        "the refusal came before any commit was attempted",
    );
    assert!(library.holds_container(row.container_id));
}
