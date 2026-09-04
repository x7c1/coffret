use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKind};

use crate::conformance_library::Library;
use crate::entry_paths::entry_path;
use crate::freeze_conformance::counting_store::CountingStore;
use crate::freeze_conformance::fixtures::{
    container_handle, filler, freeze, freeze_against, keys, map, opened, spooled, sync_source,
    write, TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;

/// The files a case syncs before freezing them.
fn files() -> Vec<(String, Vec<u8>)> {
    (0..9)
        .map(|index| {
            (
                format!("albums/2026/{index:03}.jpg"),
                filler(50 + (index * 41) % 130, 0x40 + index as u8),
            )
        })
        .collect()
}

/// One-file Containers a sync uploaded are absorbed into Packs, and their
/// objects go.
///
/// This is the second half of PK-1: a local file whose current Entry is held by
/// a one-file Container is eligible, and the Pack that takes it in displaces
/// exactly that Container. One Journal batch carries both halves — the Packs in
/// additions, the absorbed Containers in removals — which is what lets each
/// Entry Path move from its old Container to its new Pack within a single record
/// (spec: PK-7, CP-14, EP-6). The absorbed objects are trashed after the record
/// exists, so they leave the listing and stay recoverable.
pub async fn previously_synced_containers_are_absorbed(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let files = files();
    for (relative, content) in &files {
        write(fixture.source_folder(), relative, content).await;
    }
    let synced = sync_source(fixture, &keys, 1).await;
    assert_eq!(
        synced.added.len(),
        files.len(),
        "one Container per file (spec: PK-15)",
    );
    let before: BTreeSet<ContainerId> = synced.added.iter().copied().collect();

    let outcome = freeze(fixture, &keys, TARGET, 2).await;

    assert!(outcome.packs.len() > 1, "the folder is cut more than once");
    assert_eq!(outcome.frozen_entries(), files.len());
    assert_eq!(
        outcome.absorbed.iter().copied().collect::<BTreeSet<_>>(),
        before,
        "the removals are exactly the one-file Containers the Packs replace \
         (spec: PK-7)",
    );
    assert!(outcome.surfaced.is_empty());

    let commit = outcome
        .commit
        .expect("absorbing nine Containers is worth a commit");
    assert_eq!(
        commit
            .record
            .removals
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        before,
    );
    assert_eq!(commit.record.additions.len(), outcome.packs.len());
    assert!(
        commit.untrashed.is_empty(),
        "the absorbed Containers' objects were trashed",
    );

    let library = Library::read(store).await;
    for container_id in &before {
        assert!(
            !library.holds_container(*container_id),
            "an absorbed Container's object leaves the listing",
        );
    }

    // Every Entry now stands in a Pack, and the bytes are the ones that were on
    // disk all along.
    for (relative, content) in &files {
        let location = index
            .entry_at(&entry_path(relative.clone()))
            .await
            .expect("asking the catalog for a path must succeed")
            .expect("the Entry is current");
        assert!(
            !before.contains(&location.container_id),
            "{relative} moved out of the Container that held it",
        );
        let decoded = opened(store, &commit.record, location.container_id).await;
        assert_eq!(decoded.kind, ContainerKind::Pack);
        let entry = decoded
            .entries
            .iter()
            .find(|entry| entry.metadata.path.as_str() == relative)
            .expect("the Pack the catalog names holds the Entry");
        assert_eq!(&entry.content, content);
    }
    assert_eq!(spooled(fixture.spool()).await, 0);
}

/// An immediately repeated freeze selects nothing and touches no existing Pack.
///
/// `freeze` persists no folder state, so the second run is not remembering
/// anything — it simply finds every Entry already held by a Pack, which is never
/// eligible (spec: PK-1, PK-2). That is a claim about a cost and an absence as
/// much as about an outcome: an existing Pack must not be read as input and must
/// not be rewritten, and nothing the flow returns would say so, which is why the
/// case names the objects the run touched instead.
///
/// Nothing is committed either. A Journal record is a generation, and spending
/// one on a batch that changes no Container would make every device replay a
/// record that says nothing (spec: CP-1).
pub async fn a_repeated_freeze_selects_nothing_and_leaves_packs_untouched(
    fixture: &FreezeUnderTest,
) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;

    let files = files();
    for (relative, content) in &files {
        write(fixture.source_folder(), relative, content).await;
    }
    let first = freeze(fixture, &keys, TARGET, 1).await;
    let head = first
        .commit
        .expect("a folder of new files is worth a commit")
        .record
        .generation;

    let library = Library::read(fixture.store()).await;
    let counting = CountingStore::around(fixture.store());
    let second = freeze_against(&counting, fixture, &keys, TARGET, 2).await;

    assert!(second.packs.is_empty(), "there was nothing to pack");
    assert!(second.absorbed.is_empty());
    assert!(
        second.surfaced.is_empty(),
        "an Entry a Pack already holds and the file still matches is not a finding",
    );
    assert_eq!(second.packed_already, files.len());
    assert!(second.commit.is_none(), "there was nothing to commit");
    assert_eq!(counting.writes(), 0, "the run wrote nothing at all");

    for pack in &first.packs {
        assert!(
            !counting.wrote(pack.container_id),
            "a freeze never rewrites an existing Pack (spec: PK-2)",
        );
        let object = container_handle(fixture.store(), pack.container_id).await;
        assert!(
            !counting.read_object(&object),
            "a freeze never reads an existing Pack as input (spec: PK-1)",
        );
        assert!(
            library.holds_container(pack.container_id),
            "the Pack's object is where the first run left it",
        );
    }

    let checkpoint = fixture
        .source()
        .checkpoint()
        .await
        .expect("reading the checkpoint must succeed")
        .expect("the first freeze committed");
    assert_eq!(
        checkpoint.head_generation, head,
        "the Library's head is where the first freeze left it",
    );
    assert_eq!(spooled(fixture.spool()).await, 0);
}
