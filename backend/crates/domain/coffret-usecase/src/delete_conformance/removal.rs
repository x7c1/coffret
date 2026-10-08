use std::collections::BTreeSet;

use crate::commit_conformance::faulty_store::FaultyStore;
use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::delete_conformance::fixtures::{
    current, current_containers, delete, folder, freeze, lose_key, map, paths, stored, sync, write,
};
use crate::entry_paths::entry_path;

/// PK-9, CP-1, CP-14: deleting the Entry a one-file Container holds commits one
/// batch that removes that Container and adds nothing, and the object goes to
/// the provider's trash after the commit.
///
/// The Container holding the other file is untouched: a deletion is about the
/// Containers that hold a named Entry and no others.
pub async fn an_entry_in_a_one_file_container_is_deleted_by_removing_it(fixture: &DeleteUnderTest) {
    let index = fixture.index();
    map(fixture).await;
    write(fixture, "albums/a.jpg", b"the first file");
    write(fixture, "albums/b.jpg", b"the second file");
    sync(fixture, 1).await;
    let doomed = current(index, "albums/a.jpg")
        .await
        .expect("the sync put the file in the Library")
        .container_id;
    let other = current(index, "albums/b.jpg")
        .await
        .expect("the sync put the file in the Library")
        .container_id;

    let outcome = delete(fixture.store(), fixture, paths(&["albums/a.jpg"]), 2).await;

    let commit = outcome
        .commit
        .as_ref()
        .expect("a deletion is worth a commit");
    assert_eq!(
        commit.record.removals(),
        [doomed],
        "the one Container holding the Entry is removed (spec: PK-9)",
    );
    assert!(
        commit.record.additions().is_empty(),
        "nothing replaces a Container with nothing left to keep (spec: PK-10)",
    );
    assert!(commit.untrashed.is_empty());
    assert_eq!(outcome.deleted, vec![entry_path("albums/a.jpg")]);
    assert_eq!(outcome.bytes, b"the first file".len() as u64);
    assert_eq!(outcome.removed, vec![doomed]);
    assert!(outcome.rebuilt.is_empty());
    assert!(outcome.refused.is_empty());

    assert!(current(index, "albums/a.jpg").await.is_none());
    assert_eq!(
        current(index, "albums/b.jpg")
            .await
            .map(|at| at.container_id),
        Some(other),
        "the other file's Container is untouched",
    );
    assert!(
        !stored(fixture.store(), doomed).await,
        "the removed object went to the trash after the commit (spec: OC-6)",
    );
    assert!(stored(fixture.store(), other).await);
}

/// PK-9, KL-17: a Pack all of whose Entries are named goes to removals — and so
/// does one whose key is lost, because a genuine committed removal is exactly
/// how a key-lost Container leaves the current set.
///
/// Nothing has to be read to remove a Container, so the lost key costs nothing
/// here; the next Keyring generation simply stops mapping it (spec: KL-7).
pub async fn a_pack_whose_entries_are_all_deleted_is_removed_even_with_its_key_lost(
    fixture: &DeleteUnderTest,
) {
    let index = fixture.index();
    map(fixture).await;
    write(fixture, "albums/a.jpg", b"one");
    write(fixture, "albums/b.jpg", b"two");
    let readable = freeze(fixture, 1).await;
    write(fixture, "books/x.pdf", b"three");
    write(fixture, "books/y.pdf", b"four");
    let unreadable = freeze(fixture, 2).await;
    lose_key(fixture.store(), index, unreadable).await;

    let outcome = delete(
        fixture.store(),
        fixture,
        paths(&["albums/a.jpg", "albums/b.jpg"]).and_folder(entry_path("books")),
        3,
    )
    .await;

    let commit = outcome
        .commit
        .as_ref()
        .expect("a deletion is worth a commit");
    let removals: BTreeSet<_> = commit.record.removals().iter().copied().collect();
    assert_eq!(removals, BTreeSet::from([readable, unreadable]));
    assert!(commit.record.additions().is_empty());
    assert!(
        outcome.refused.is_empty(),
        "a key-lost Pack deleted whole is not refused (spec: KL-17)",
    );
    assert_eq!(outcome.deleted.len(), 4);
    assert!(
        current_containers(index).await.is_empty(),
        "both Packs left the current set, the key-lost one by a genuine removal",
    );
    assert!(!stored(fixture.store(), readable).await);
    assert!(!stored(fixture.store(), unreadable).await);
}

/// CP-1, OC-6: a removed object that will not go to the trash leaves the
/// deletion committed, and is reported rather than lost.
///
/// The record is already the truth about what is current, so a trash refusal
/// un-commits nothing; it leaves an object no current state names, which a
/// later run can still move (spec: CP-14).
pub async fn a_removal_that_will_not_go_to_the_trash_leaves_the_deletion_committed(
    fixture: &DeleteUnderTest,
) {
    let index = fixture.index();
    map(fixture).await;
    write(fixture, "albums/a.jpg", b"the only file");
    write(fixture, "albums/b.jpg", b"another file");
    let pack = freeze(fixture, 1).await;

    let refusing = FaultyStore::refusing_to_trash(fixture.store());
    let outcome = delete(&refusing, fixture, folder("albums"), 2).await;

    let commit = outcome.commit.expect("the deletion committed all the same");
    assert_eq!(commit.record.removals(), [pack]);
    assert_eq!(
        commit
            .untrashed
            .iter()
            .map(|removal| removal.container_id)
            .collect::<Vec<_>>(),
        vec![pack],
        "the object the trash refused is reported with the commit",
    );
    assert!(current(index, "albums/a.jpg").await.is_none());
    assert!(
        current_containers(index).await.is_empty(),
        "the commit stands: the Pack is not current",
    );
    assert!(
        stored(fixture.store(), pack).await,
        "and its object is still on Storage, for a later run to trash",
    );
}
