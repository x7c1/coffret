use std::collections::BTreeSet;

use coffret_model::ContainerId;

use crate::commit::CommitError;
use crate::conformance_library::Library;
use crate::entry_paths::entry_path;
use crate::freeze::{freeze_folder, FreezeError};
use crate::freeze_conformance::fixtures::{
    filler, keys, map, pending, policy, request, sync_source, write, ROOMY_TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;
use crate::removing_rival::RemovingRival;

/// A Pack absorbing a one-file Container another device removed while this run
/// was packing it is refused, and the removal stands (spec: CP-7, CP-18).
///
/// The run read the head, selected three one-file Containers, and uploaded the
/// Pack that absorbs them. Before its commit caught up again, another device
/// removed one of them. Committing the Pack anyway would bring the deleted
/// Entry back inside it, so the commit refuses the batch as a conflict naming
/// that Container: the head stays the other device's, the Containers the Pack
/// would have absorbed stay where they were, and the uploaded Pack is left for
/// the next sync to settle like any upload whose batch never committed
/// (spec: OC-2, OC-3).
pub async fn a_pack_absorbing_a_removed_container_is_refused(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let relatives = ["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"];
    for (seed, relative) in relatives.iter().enumerate() {
        write(
            fixture.fs(),
            fixture.source_folder(),
            relative,
            &filler(60, 0x50 + seed as u8),
        );
    }
    let synced = sync_source(fixture, &keys, 1).await;
    let before: BTreeSet<ContainerId> = synced.added.iter().copied().collect();
    assert_eq!(before.len(), relatives.len());

    let gone = index
        .entry_at(&entry_path("albums/a.jpg"))
        .await
        .expect("asking the catalog for a path must succeed")
        .expect("the synced Entry is current")
        .container_id;

    let rival = RemovingRival::removing(store, keys.control(), policy(), vec![gone]);
    let result = freeze_folder(request(&rival, index, &keys, fixture.fs(), ROOMY_TARGET, 2)).await;
    assert!(rival.has_removed(), "the other device removed it mid-run");

    match result {
        Err(FreezeError::Commit(failure)) => match *failure.error {
            CommitError::RemovalNotCurrent { ref container_ids } => {
                assert_eq!(container_ids, &vec![gone]);
            }
            ref other => panic!("expected the stale removal to be refused, got {other:?}"),
        },
        other => panic!("expected the commit to refuse the batch, got {other:?}"),
    }

    let library = Library::read(store).await;
    assert_eq!(
        library.heads(),
        2,
        "the sync's head and the other device's, and none of this run's",
    );
    assert!(
        index
            .entry_at(&entry_path("albums/a.jpg"))
            .await
            .expect("asking the catalog for a path must succeed")
            .is_none(),
        "the removed Entry does not come back inside a Pack",
    );
    for relative in &relatives[1..] {
        let location = index
            .entry_at(&entry_path(*relative))
            .await
            .expect("asking the catalog for a path must succeed")
            .expect("an Entry nobody removed is still current");
        assert!(
            before.contains(&location.container_id),
            "{relative} stays in the one-file Container that held it",
        );
    }

    let rows = pending(index).await;
    assert!(!rows.is_empty(), "the uploaded Pack keeps its row");
    for row in &rows {
        assert!(
            !row.commit_attempted,
            "the refusal came before any commit was attempted",
        );
        assert!(library.holds_container(row.container_id));
    }
}
