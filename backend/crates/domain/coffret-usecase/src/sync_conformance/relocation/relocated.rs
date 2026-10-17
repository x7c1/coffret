use crate::device_state::{LocalEntryState, LocalObservation};
use crate::entry_paths::entry_path;
use crate::freeze::{freeze_folder, FreezeRequest};
use crate::relocate::{Relocation, RelocationRefused};
use crate::sync_conformance::fixtures::{at as clock, keys, observed, policy, spool_dir};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

use super::{
    at, carry_in, catch_up_index, content, refused, relocated, rename, row, sync, uploaded_nothing,
};

/// The file of an Entry another device renamed is relocated to the Entry's new path,
/// the folders the new path needs are made and the folder it left empty is
/// removed, and its record follows it: the run uploads nothing, and the next
/// one finds the file unchanged at its Entry's own path (spec: EP-10, EP-15).
pub async fn a_renamed_entrys_file_is_relocated_to_follow_it(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("inbox/a.jpg", b"the file's bytes")]).await;
    rename(fixture, &[("inbox/a.jpg", "albums/2026/a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert_eq!(
        relocated(&outcome),
        [("inbox/a.jpg".to_owned(), "albums/2026/a.jpg".to_owned())]
    );
    assert!(outcome.surfaced.is_empty(), "{:?}", outcome.surfaced);
    assert_eq!(
        content(fixture, "albums/2026/a.jpg").as_deref(),
        Some(&b"the file's bytes"[..])
    );
    assert!(!fixture.fs().holds(&at(fixture, "inbox/a.jpg")));
    assert!(
        !fixture.fs().holds(&at(fixture, "inbox")),
        "the folder the file left empty is removed"
    );
    let relocated = row(fixture, "albums/2026/a.jpg")
        .await
        .expect("the record follows the file");
    assert_eq!(relocated.state, LocalEntryState::Present);
    assert_eq!(relocated.relocate_to, None);
    assert!(row(fixture, "inbox/a.jpg").await.is_none());
    assert!(fixture.fs().discarded().is_empty());

    let again = sync(fixture, 3).await;
    uploaded_nothing(&again);
    assert!(again.relocations.is_empty(), "nothing waits any more");
    assert_eq!(again.unchanged, 1, "the file is its Entry's own copy");
}

/// The Entries of a Pack this device froze, renamed into another folder as a
/// folder's worth, are each relocated, and the folder they left is removed
/// (spec: EP-10, PK-7).
pub async fn a_folders_worth_of_pack_entries_is_relocated(fixture: &SyncUnderTest) {
    let volumes: [(&str, &[u8]); 3] = [
        ("books/vol1.txt", b"volume one"),
        ("books/vol2.txt", b"volume two, longer"),
        ("books/vol3.txt", b"volume three, the longest"),
    ];
    carry_in(fixture, &volumes).await;
    let keys = keys();
    let frozen = freeze_folder(
        FreezeRequest::new(
            fixture.store(),
            fixture.index(),
            &keys,
            fixture.fs(),
            fixture.fs(),
            spool_dir(),
            1 << 20,
            crate::device_state::BatchId::new("freeze-2"),
            clock(2),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("the freeze must succeed: {error}"));
    assert_eq!(frozen.frozen_entries(), 3, "the volumes land in one Pack");
    rename(
        fixture,
        &[
            ("books/vol1.txt", "library/books/vol1.txt"),
            ("books/vol2.txt", "library/books/vol2.txt"),
            ("books/vol3.txt", "library/books/vol3.txt"),
        ],
    )
    .await;

    let outcome = sync(fixture, 3).await;

    uploaded_nothing(&outcome);
    assert_eq!(
        relocated(&outcome),
        volumes.map(|(path, _)| (path.to_owned(), format!("library/{path}"))),
    );
    for (path, bytes) in volumes {
        assert_eq!(
            content(fixture, &format!("library/{path}")).as_deref(),
            Some(bytes)
        );
    }
    assert!(
        !fixture.fs().holds(&at(fixture, "books")),
        "the folder the volumes left is removed"
    );
    assert_eq!(sync(fixture, 4).await.unchanged, 3);
}

/// Two files whose Entries swapped their names exchange them, and three whose
/// Entries rotated theirs rotate with them: no file of either can be relocated first
/// while another stands where it goes, so each is exchanged into place, and
/// nothing is overwritten or left behind (spec: EP-10, FM-15).
pub async fn files_whose_entries_traded_names_exchange_them(fixture: &SyncUnderTest) {
    let files: [(&str, &[u8]); 5] = [
        ("a.jpg", b"a"),
        ("b.jpg", b"bb"),
        ("c.jpg", b"ccc"),
        ("d.jpg", b"dddd"),
        ("e.jpg", b"eeeee"),
    ];
    carry_in(fixture, &files).await;
    rename(
        fixture,
        &[
            ("a.jpg", "b.jpg"),
            ("b.jpg", "a.jpg"),
            ("c.jpg", "d.jpg"),
            ("d.jpg", "e.jpg"),
            ("e.jpg", "c.jpg"),
        ],
    )
    .await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert_eq!(
        relocated(&outcome),
        [
            ("a.jpg", "b.jpg"),
            ("b.jpg", "a.jpg"),
            ("c.jpg", "d.jpg"),
            ("d.jpg", "e.jpg"),
            ("e.jpg", "c.jpg"),
        ]
        .map(|(from, to)| (from.to_owned(), to.to_owned())),
    );
    for (relative, bytes) in [
        ("b.jpg", &b"a"[..]),
        ("a.jpg", b"bb"),
        ("d.jpg", b"ccc"),
        ("e.jpg", b"dddd"),
        ("c.jpg", b"eeeee"),
    ] {
        assert_eq!(
            content(fixture, relative).as_deref(),
            Some(bytes),
            "{relative}"
        );
    }
    assert!(fixture.fs().discarded().is_empty());
    let again = sync(fixture, 3).await;
    assert!(again.relocations.is_empty());
    assert_eq!(again.unchanged, 5, "every file is its Entry's own copy");
}

/// An Entry renamed and renamed back before this device's next run leaves its
/// file where it is, waiting for nothing (spec: EP-10, FM-15).
pub async fn a_rename_back_leaves_nothing_to_relocate(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", b"the file's bytes")]).await;
    rename(fixture, &[("a.jpg", "b.jpg")]).await;
    catch_up_index(fixture, fixture.index()).await;
    rename(fixture, &[("b.jpg", "a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert!(outcome.relocations.is_empty(), "{:?}", outcome.relocations);
    assert_eq!(outcome.unchanged, 1);
    assert_eq!(
        row(fixture, "a.jpg")
            .await
            .expect("the file keeps its row")
            .relocate_to,
        None
    );
}

/// Where this device already has its own copy of the Entry at the Entry's new
/// path — a fetch placed it there while the old one waited — the waiting file
/// is redundant: unedited, it is discarded into the trash and its record
/// forgotten; edited, it is kept and reported, and nothing is overwritten
/// either way (spec: EP-10, EP-15).
pub async fn a_file_already_placed_at_the_new_path_makes_the_waiting_one_redundant(
    fixture: &SyncUnderTest,
) {
    carry_in(
        fixture,
        &[("a.jpg", b"the first file"), ("c.jpg", b"the second file")],
    )
    .await;
    rename(fixture, &[("a.jpg", "b.jpg"), ("c.jpg", "d.jpg")]).await;
    catch_up_index(fixture, fixture.index()).await;
    // What a fetch of each Entry to its new path leaves: the Entry's bytes at
    // the new path, recorded as this device's copy of it.
    for (old, new, bytes) in [
        ("a.jpg", "b.jpg", &b"the first file"[..]),
        ("c.jpg", "d.jpg", b"the second file"),
    ] {
        let placed = at(fixture, new);
        fixture.fs().write_file(&placed, bytes);
        let (size, mtime) = observed(fixture.fs(), &placed);
        let waiting = row(fixture, old).await.expect("the old file waits");
        fixture
            .index()
            .mark_present(LocalObservation {
                path: entry_path(new),
                size,
                mtime,
                at: clock(2),
                entry: waiting.observation.entry,
                hash: waiting.observation.hash,
            })
            .await
            .expect("recording a fetched file must succeed");
    }
    fixture
        .fs()
        .write_file(&at(fixture, "c.jpg"), b"the second file, edited");

    let outcome = sync(fixture, 3).await;

    uploaded_nothing(&outcome);
    let mut discarded = Vec::new();
    let mut kept = Vec::new();
    for one in &outcome.relocations {
        match one {
            Relocation::Discarded { from, to } => discarded.push((from.clone(), to.clone())),
            Relocation::Refused {
                from,
                reason: RelocationRefused::Edited,
                ..
            } => kept.push(from.clone()),
            other => panic!("unexpected relocation {other:?}"),
        }
    }
    assert_eq!(discarded, [(entry_path("a.jpg"), entry_path("b.jpg"))]);
    assert_eq!(kept, [entry_path("c.jpg")]);
    assert_eq!(fixture.fs().discarded(), [at(fixture, "a.jpg")]);
    assert!(row(fixture, "a.jpg").await.is_none());
    assert_eq!(
        content(fixture, "c.jpg").as_deref(),
        Some(&b"the second file, edited"[..]),
        "the edited copy is kept"
    );
    assert_eq!(
        row(fixture, "c.jpg")
            .await
            .expect("and still waits")
            .relocate_to,
        Some(entry_path("d.jpg"))
    );
    for (relative, bytes) in [
        ("b.jpg", &b"the first file"[..]),
        ("d.jpg", b"the second file"),
    ] {
        assert_eq!(content(fixture, relative).as_deref(), Some(bytes));
    }

    // The edited one is reported every run, and refused the same way.
    let again = sync(fixture, 4).await;
    let (from, _, reason) = refused(&again);
    assert_eq!(from, "c.jpg");
    assert!(matches!(reason, RelocationRefused::Edited), "{reason:?}");
}
