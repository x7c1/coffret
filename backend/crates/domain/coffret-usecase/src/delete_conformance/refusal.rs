use crate::delete::{PackRefusal, Unverified};
use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::delete_conformance::fixtures::{
    container_handle, current, delete, freeze, lose_key, map, paths, spooled, stored, sync, write,
};
use crate::entry_paths::entry_path;
use crate::fetch_conformance::mangling_store::ManglingStore;

/// PK-10: a Pack that keeps Entries and does not verify when it is read is left
/// exactly as it was — no replacement, no removal, every Entry of it still
/// current — and the refusal says which Pack, which Entries it kept and spared,
/// and why. The rest of the deletion commits.
///
/// The damage is done in transit, one byte of the object, so the Library
/// itself is sound and what is being checked is that a rebuild holds what it
/// read to what the record says before writing a word of a replacement
/// (spec: FM-15, CP-11). A replacement written from it would claim to carry
/// Entries this device could not vouch for.
pub async fn a_pack_that_does_not_verify_is_refused_and_the_rest_commits(
    fixture: &DeleteUnderTest,
) {
    let store = fixture.store();
    let index = fixture.index();
    map(fixture).await;
    write(
        fixture,
        "albums/a.jpg",
        b"kept, and unreadable as it arrives",
    );
    write(fixture, "albums/b.jpg", b"named, and spared with its Pack");
    let pack = freeze(fixture, 1).await;
    write(
        fixture,
        "notes/n.txt",
        b"a one-file Entry deleted beside it",
    );
    sync(fixture, 2).await;
    let one_file = current(index, "notes/n.txt")
        .await
        .expect("the sync put the file in the Library")
        .container_id;

    let mangling = ManglingStore::around(store, container_handle(store, pack).await);
    let outcome = delete(
        &mangling,
        fixture,
        paths(&["albums/b.jpg", "notes/n.txt"]),
        3,
    )
    .await;

    assert_eq!(outcome.refused.len(), 1, "the Pack is refused");
    let refused = &outcome.refused[0];
    assert_eq!(refused.container_id, pack);
    assert_eq!(refused.kept, vec![entry_path("albums/a.jpg")]);
    assert_eq!(refused.spared, vec![entry_path("albums/b.jpg")]);
    assert!(
        matches!(
            refused.reason,
            PackRefusal::Unverified(Unverified::CiphertextMismatch { .. })
        ),
        "the object is not the one its record names (spec: FM-15), got {:?}",
        refused.reason,
    );

    let commit = outcome
        .commit
        .as_ref()
        .expect("the one-file Entry's removal commits all the same");
    assert_eq!(
        commit.record.removals(),
        [one_file],
        "nothing is committed for the refused Pack (spec: PK-10)",
    );
    assert!(commit.record.additions().is_empty(), "and no replacement");
    assert!(outcome.rebuilt.is_empty());
    assert_eq!(outcome.deleted, vec![entry_path("notes/n.txt")]);

    for path in ["albums/a.jpg", "albums/b.jpg"] {
        assert_eq!(
            current(index, path).await.map(|at| at.container_id),
            Some(pack),
            "{path} stays where it was",
        );
    }
    assert!(stored(store, pack).await, "and so does the Pack's object");
    assert_eq!(
        spooled(fixture.fs()),
        0,
        "the replacement it had begun is gone from the spool"
    );
    assert!(
        index
            .pending_rows()
            .await
            .expect("asking for pending rows must succeed")
            .is_empty(),
        "and so is the row that named it (spec: OC-2, OC-8)",
    );
}

/// PK-10, KL-17: deleting some of a key-lost Pack's Entries is refused, because
/// keeping the others means reading them back and nothing can. The refusal
/// names the Entries the Pack keeps, and nothing is committed for it — no
/// partial copy of an unreadable Pack is invented.
///
/// Deleting all of them is allowed and is a different case, beside the one-file
/// removals: a key-lost Container leaves the current set by a genuine removal
/// (spec: KL-17).
pub async fn a_partial_deletion_of_a_key_lost_pack_is_refused(fixture: &DeleteUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    map(fixture).await;
    write(fixture, "albums/a.jpg", b"first");
    write(fixture, "albums/b.jpg", b"second");
    write(fixture, "albums/c.jpg", b"third");
    let pack = freeze(fixture, 1).await;
    lose_key(store, index, pack).await;

    let outcome = delete(store, fixture, paths(&["albums/b.jpg"]), 2).await;

    assert_eq!(outcome.refused.len(), 1);
    let refused = &outcome.refused[0];
    assert_eq!(refused.container_id, pack);
    assert!(matches!(refused.reason, PackRefusal::KeyLost));
    assert_eq!(
        refused.kept,
        vec![entry_path("albums/a.jpg"), entry_path("albums/c.jpg")],
        "the refusal names the Entries the Pack keeps",
    );
    assert_eq!(refused.spared, vec![entry_path("albums/b.jpg")]);
    assert!(
        outcome.commit.is_none(),
        "nothing else was asked for, so nothing is committed (spec: CP-1)",
    );
    assert!(outcome.deleted.is_empty());
    assert!(current(index, "albums/b.jpg").await.is_some());
    assert!(stored(store, pack).await);
    assert_eq!(spooled(fixture.fs()), 0, "nothing was spooled for it");
}
