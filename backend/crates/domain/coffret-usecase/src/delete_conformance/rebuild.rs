use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKind, EntryMetadata};

use crate::delete_conformance::delete_under_test::DeleteUnderTest;
use crate::delete_conformance::fixtures::{
    current, current_containers, delete, filler, folder, freeze, map, opened, paths, spooled,
    stored, table, write,
};
use crate::entry_paths::entry_path;

/// PK-10, PK-15, CP-1, CP-14: deleting some Entries of a Pack replaces it by
/// read-modify-replace — a new Pack, under a new Container ID, holding exactly
/// the kept Entries with the metadata the old one recorded for them and in the
/// order it held them, the old Pack in removals and the new one in additions of
/// one batch.
///
/// The replacement is opened the long way round, under the envelope the
/// committed Keyring maps it to, so the claim is about what another device
/// finds rather than what the run reported. Nothing of the kept Entries'
/// metadata is re-derived — paths, `original_mtime`, `original_btime`, hashes —
/// because a replacement that wrote down a different time for a file it never
/// saw would be inventing one.
pub async fn a_pack_that_keeps_entries_is_rebuilt_with_exactly_them(fixture: &DeleteUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    map(fixture).await;
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("albums/a.jpg", filler(300, 1)),
        ("albums/b.jpg", filler(120, 2)),
        ("albums/c.jpg", Vec::new()),
        ("albums/d.jpg", filler(500, 4)),
        ("albums/e.jpg", filler(40, 5)),
    ];
    for (path, content) in &files {
        write(fixture, path, content);
    }
    let old = freeze(fixture, 1).await;
    let old_table = table(index, old).await;

    let outcome = delete(store, fixture, paths(&["albums/b.jpg", "albums/d.jpg"]), 2).await;

    let commit = outcome
        .commit
        .as_ref()
        .expect("a deletion is worth a commit");
    assert_eq!(commit.record.removals(), [old], "the old Pack is removed");
    assert_eq!(
        commit.record.additions().len(),
        1,
        "and exactly one replacement is added in the same batch (spec: CP-1)",
    );
    let replacement = commit.record.additions()[0].container().id;
    assert_ne!(
        replacement, old,
        "the replacement is a new Container (spec: CP-14)"
    );
    assert_eq!(outcome.rebuilt.len(), 1);
    assert_eq!(outcome.rebuilt[0].replaced, old);
    assert_eq!(outcome.rebuilt[0].container_id, replacement);
    assert_eq!(outcome.rebuilt[0].kept, 3);
    assert_eq!(outcome.rebuilt[0].omitted, 2);
    assert_eq!(
        outcome.deleted,
        vec![entry_path("albums/b.jpg"), entry_path("albums/d.jpg")],
    );
    assert_eq!(outcome.bytes, 120 + 500);
    assert!(outcome.refused.is_empty());

    let kept: Vec<EntryMetadata> = old_table
        .iter()
        .filter(|row| !["albums/b.jpg", "albums/d.jpg"].contains(&row.path.as_str()))
        .cloned()
        .collect();
    let decoded = opened(store, &commit.record, replacement).await;
    assert_eq!(
        decoded.kind,
        ContainerKind::Pack,
        "Pack-ness survives the replacement (spec: PK-15)"
    );
    assert_eq!(
        decoded.entries.len(),
        kept.len(),
        "exactly the kept Entries, and no deleted one",
    );
    for (row, carried) in kept.iter().zip(&decoded.entries) {
        let metadata = &carried.metadata;
        assert_eq!(
            metadata.path, row.path,
            "in the order the old Pack held them"
        );
        assert_eq!(
            metadata.mtime, row.mtime,
            "with the recorded modification time"
        );
        assert_eq!(metadata.btime, row.btime, "and the recorded birth time");
        assert_eq!(metadata.hash, row.hash, "and the recorded hash");
        assert_eq!(metadata.extent.size(), row.extent.size());
        let original = &files
            .iter()
            .find(|(path, _)| *path == row.path.as_str())
            .expect("every kept Entry is a file the case wrote")
            .1;
        assert_eq!(&carried.content, original, "byte for byte");
    }
    assert_eq!(
        commit.record.additions()[0].entries(),
        decoded
            .entries
            .iter()
            .map(|entry| entry.metadata.clone())
            .collect::<Vec<_>>(),
        "the record carries the table the replacement's meta section does (spec: CP-11)",
    );

    for path in ["albums/a.jpg", "albums/c.jpg", "albums/e.jpg"] {
        assert_eq!(
            current(index, path).await.map(|at| at.container_id),
            Some(replacement),
            "{path} is current in the replacement",
        );
    }
    for path in ["albums/b.jpg", "albums/d.jpg"] {
        assert!(
            current(index, path).await.is_none(),
            "{path} left the Library"
        );
    }
    assert!(!stored(store, old).await, "the old Pack went to the trash");
    assert_eq!(spooled(fixture.fs()), 0, "and nothing is left in the spool");
    assert!(
        index
            .pending_rows()
            .await
            .expect("asking for pending rows must succeed")
            .is_empty(),
        "and nothing is left pending (spec: OC-2)",
    );
    assert!(
        index
            .local_entry_at(&entry_path("albums/a.jpg"))
            .await
            .expect("asking for a local row must succeed")
            .is_some(),
        "the device's own record of a file it holds is untouched",
    );

    // Asking again for what has already gone deletes nothing: the paths hold no
    // current Entry, so no Container is examined — neither the removed Pack,
    // which never comes back (spec: CP-14), nor the replacement — and nothing
    // is committed (spec: CP-1).
    let again = delete(store, fixture, paths(&["albums/b.jpg", "albums/d.jpg"]), 3).await;
    assert!(
        again.commit.is_none(),
        "a repeated deletion commits nothing"
    );
    assert!(again.deleted.is_empty());
    assert!(again.removed.is_empty() && again.rebuilt.is_empty() && again.refused.is_empty());
    assert_eq!(
        again.missing,
        vec![entry_path("albums/b.jpg"), entry_path("albums/d.jpg")],
        "and says the paths it was given name nothing",
    );
    assert_eq!(
        current_containers(index).await,
        BTreeSet::from([replacement]),
        "the replacement is left as the first deletion made it",
    );
}

/// PK-9: deleting a folder examines every Pack holding an Entry under it —
/// however many, because Packs from different invocations overlap and
/// interleave (spec: PK-8) — and commits the whole of it as one batch: the Packs
/// holding nothing else are removed, and each Pack that also holds Entries
/// outside the folder is rebuilt around them.
pub async fn a_folder_spanning_several_packs_is_deleted_in_one_batch(fixture: &DeleteUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    map(fixture).await;

    // Three invocations, three Packs, and the folder in all of them: the first
    // two mixed with Entries outside it, the third holding it alone.
    write(fixture, "albums/a.jpg", &filler(100, 1));
    write(fixture, "albums/b.jpg", &filler(110, 2));
    write(fixture, "books/x.pdf", &filler(120, 3));
    let mixed_first = freeze(fixture, 1).await;
    write(fixture, "albums/c.jpg", &filler(130, 4));
    write(fixture, "zines/z.jpg", &filler(140, 5));
    let mixed_second = freeze(fixture, 2).await;
    write(fixture, "albums/d.jpg", &filler(150, 6));
    write(fixture, "albums/e.jpg", &filler(160, 7));
    let albums_only = freeze(fixture, 3).await;

    let outcome = delete(store, fixture, folder("albums"), 4).await;

    let commit = outcome
        .commit
        .as_ref()
        .expect("a deletion is worth a commit");
    let removals: BTreeSet<ContainerId> = commit.record.removals().iter().copied().collect();
    assert_eq!(
        removals,
        BTreeSet::from([mixed_first, mixed_second, albums_only]),
        "every Pack holding an Entry under the folder leaves, in one batch",
    );
    assert_eq!(
        commit.record.additions().len(),
        2,
        "the two mixed Packs are replaced, and the one holding only the folder is not",
    );
    let additions: BTreeSet<ContainerId> = commit
        .record
        .additions()
        .iter()
        .map(|addition| addition.container().id)
        .collect();
    assert!(additions.is_disjoint(&removals));
    assert_eq!(outcome.removed, vec![albums_only]);
    assert_eq!(
        outcome
            .rebuilt
            .iter()
            .map(|pack| pack.replaced)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([mixed_first, mixed_second]),
    );
    assert_eq!(
        outcome.deleted,
        [
            "albums/a.jpg",
            "albums/b.jpg",
            "albums/c.jpg",
            "albums/d.jpg",
            "albums/e.jpg"
        ]
        .map(entry_path)
        .to_vec(),
    );

    for (path, seed, len) in [("books/x.pdf", 3, 120), ("zines/z.jpg", 5, 140)] {
        let location = current(index, path)
            .await
            .unwrap_or_else(|| panic!("{path} is kept"));
        assert!(
            additions.contains(&location.container_id),
            "{path} is carried by a replacement",
        );
        let decoded = opened(store, &commit.record, location.container_id).await;
        assert_eq!(
            decoded.entries.len(),
            1,
            "a replacement holds only what it kept"
        );
        assert_eq!(decoded.entries[0].content, filler(len, seed));
        assert_eq!(decoded.kind, ContainerKind::Pack);
    }
    assert_eq!(
        current_containers(index).await,
        additions,
        "what is current is exactly the two replacements",
    );
    for old in [mixed_first, mixed_second, albums_only] {
        assert!(
            !stored(store, old).await,
            "every removed Pack went to the trash"
        );
    }
}
