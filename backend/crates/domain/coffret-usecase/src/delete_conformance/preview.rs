use std::collections::BTreeSet;

use crate::catch_up::{catch_up_catalog, CatchUpRequest};
use crate::delete::{committed_key_lost, preview_delete, PackRefusal};
use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::delete_conformance::fixtures::{
    current, delete, filler, freeze, lose_key, map, paths, sync, write,
};
use crate::entry_paths::entry_path;

/// PK-9: a preview of a deletion counts what the run then does — the Entries
/// and bytes that leave, the Containers removed outright, the Packs rebuilt and
/// what rebuilding them reads and writes, the Packs refused, the paths that
/// name nothing — and it does so from the catalog alone.
///
/// "From the catalog alone" is the signature's to say and this case's to hold
/// it to: the preview is handed no store and no key, so it cannot reach Storage
/// or open anything, and the catalog is the same afterwards as before — same
/// state, no pending row. The one fact it is told is which Packs' keys are lost,
/// which the catalog does not hold (spec: KL-7).
pub async fn a_preview_counts_what_the_deletion_then_does(fixture: &DeleteUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    map(fixture).await;

    // A Pack kept in part, a Pack removed whole, a key-lost Pack kept in part,
    // and a one-file Container removed whole.
    write(fixture, "albums/a.jpg", &filler(200, 1));
    write(fixture, "albums/b.jpg", &filler(300, 2));
    write(fixture, "albums/c.jpg", &filler(400, 3));
    let rebuilt = freeze(fixture, 1).await;
    write(fixture, "books/x.pdf", &filler(500, 4));
    write(fixture, "books/y.pdf", &filler(600, 5));
    let removed_pack = freeze(fixture, 2).await;
    write(fixture, "comics/p.cbz", &filler(700, 6));
    write(fixture, "comics/q.cbz", &filler(800, 7));
    let key_lost = freeze(fixture, 3).await;
    write(fixture, "notes/n.txt", &filler(90, 8));
    sync(fixture, 4).await;
    let one_file = current(index, "notes/n.txt")
        .await
        .expect("the sync put the file in the Library")
        .container_id;
    lose_key(store, index, key_lost).await;
    // The run catches the catalog up before it plans, and a preview asks the
    // catalog as it stands: a caller previews over a catalog that has seen the
    // head, so the case brings it there the way any flow would.
    let keys = super::fixtures::keys();
    catch_up_catalog(CatchUpRequest::new(store, index, &keys))
        .await
        .expect("catching up must succeed");

    let selection = paths(&[
        "albums/b.jpg",
        "books/x.pdf",
        "books/y.pdf",
        "comics/p.cbz",
        "notes/n.txt",
        "nowhere/missing.jpg",
    ]);
    let before = index.snapshot().await.expect("a snapshot of the catalog");
    let preview = preview_delete(index, &selection, &BTreeSet::from([key_lost]))
        .await
        .expect("a preview reads the catalog and nothing else");
    assert_eq!(
        index.snapshot().await.expect("a snapshot of the catalog"),
        before,
        "the preview changed nothing in the catalog",
    );
    assert!(index
        .pending_rows()
        .await
        .expect("asking for pending rows must succeed")
        .is_empty());

    assert_eq!(
        preview.entries, 4,
        "b, x, y and n leave; p is spared with its Pack"
    );
    assert_eq!(preview.bytes, 300 + 500 + 600 + 90);
    assert_eq!(
        preview.removed, 2,
        "the books Pack and the one-file Container"
    );
    assert_eq!(preview.rebuilt, 1, "the albums Pack");
    assert_eq!(preview.refused.len(), 1);
    assert_eq!(preview.refused[0].container_id, key_lost);
    assert!(matches!(preview.refused[0].reason, PackRefusal::KeyLost));
    assert_eq!(preview.refused[0].kept, vec![entry_path("comics/q.cbz")]);
    assert_eq!(preview.missing, vec![entry_path("nowhere/missing.jpg")]);

    let outcome = delete(store, fixture, selection, 5).await;

    assert_eq!(preview.entries, outcome.entries());
    assert_eq!(preview.bytes, outcome.bytes);
    assert_eq!(preview.removed, outcome.removed.len());
    assert_eq!(
        outcome.removed.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([removed_pack, one_file]),
    );
    assert_eq!(preview.rebuilt, outcome.rebuilt.len());
    assert_eq!(outcome.rebuilt[0].replaced, rebuilt);
    assert_eq!(
        preview.rebuild_read,
        outcome.rebuild_read(),
        "the bytes a rebuild reads are the old Pack's, whole",
    );
    assert_eq!(
        preview.rebuild_written,
        outcome.rebuild_written(),
        "and the bytes it writes are what the replacement weighs on Storage",
    );
    assert!(preview.rebuild_read > 0 && preview.rebuild_written > 0);
    assert_eq!(
        preview
            .refused
            .iter()
            .map(|pack| pack.container_id)
            .collect::<Vec<_>>(),
        outcome
            .refused
            .iter()
            .map(|pack| pack.container_id)
            .collect::<Vec<_>>(),
    );
    assert_eq!(preview.missing, outcome.missing);
}

/// KL-7, KL-17: the set a preview is handed is read off the committed
/// Keyring the way the run reads it — after the run's own catch-up — so a
/// preview given it names exactly the Packs the run would be refused for, and
/// writes nothing to the Library on the way.
pub async fn the_committed_keyring_names_the_packs_a_preview_refuses(fixture: &DeleteUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    map(fixture).await;

    // Before anything is committed there is no Keyring and nothing lost.
    let keys = super::fixtures::keys();
    let policy = super::fixtures::policy();
    assert_eq!(
        committed_key_lost(store, index, &keys, &policy)
            .await
            .expect("an empty Library is read"),
        BTreeSet::new(),
    );

    write(fixture, "albums/a.jpg", &filler(200, 1));
    write(fixture, "albums/b.jpg", &filler(300, 2));
    let readable = freeze(fixture, 1).await;
    write(fixture, "comics/p.cbz", &filler(700, 6));
    write(fixture, "comics/q.cbz", &filler(800, 7));
    let key_lost = freeze(fixture, 2).await;
    lose_key(store, index, key_lost).await;

    let lost = committed_key_lost(store, index, &keys, &policy)
        .await
        .expect("the committed Keyring is read");
    assert_eq!(lost, BTreeSet::from([key_lost]));
    assert!(!lost.contains(&readable));

    let preview = preview_delete(index, &paths(&["albums/a.jpg", "comics/p.cbz"]), &lost)
        .await
        .expect("a preview reads the catalog");
    assert_eq!(preview.rebuilt, 1, "the readable Pack is rebuilt");
    assert_eq!(preview.refused.len(), 1);
    assert_eq!(preview.refused[0].container_id, key_lost);
    assert!(matches!(preview.refused[0].reason, PackRefusal::KeyLost));
}
