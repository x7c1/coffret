use coffret_model::{ContainerKind, ControlObjectName, EntryNumber};

use crate::commit::{commit_batch, CommitError, PendingRename, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::fixtures::{container_id, control_keys, path, prepared, request};
use crate::commit_conformance::library::Library;
use crate::commit_conformance::racing_store::RacingStore;
use crate::entry_ref::EntryRef;
use crate::generations::generation;

/// Commits one one-file Container per path, `seed` upwards, each in a batch
/// of its own, so a case starts from a Library holding Entries at `paths`.
async fn commit_each(fixture: &CommitUnderTest, seed: u8, paths: &[&str]) {
    let store = fixture.store();
    let keys = control_keys();
    for (at, one) in paths.iter().enumerate() {
        let seed = seed + at as u8;
        Library::upload_container(store, container_id(seed)).await;
        commit_batch(request(
            store,
            fixture.index(),
            &keys,
            PreparedBatch::adding(vec![prepared(seed, ContainerKind::OneFile, &[one])]),
        ))
        .await
        .expect("a batch adding a path nothing stands near must commit");
    }
}

/// The path a commit of `batch` is refused for as both a file and a folder,
/// asserting that the refused commit wrote nothing.
async fn refused_as_both(fixture: &CommitUnderTest, batch: PreparedBatch) -> String {
    let store = fixture.store();
    let keys = control_keys();
    let before = Library::read(store).await.names();
    let result = commit_batch(request(store, fixture.index(), &keys, batch))
        .await
        .map_err(|failure| *failure.error);
    let path = match result {
        Err(CommitError::FileAndFolder { path }) => path.as_str().to_owned(),
        other => panic!("expected the batch refused as a file and a folder, got {other:?}"),
    };
    assert_eq!(
        Library::read(store).await.names(),
        before,
        "a refused batch writes nothing at all",
    );
    path
}

/// The rename of the one Entry of the one-file Container `seed` names, from
/// `from` to `to`.
fn renaming(seed: u8, from: &str, to: &str) -> PendingRename {
    PendingRename {
        entry: EntryRef {
            container_id: container_id(seed),
            entry_number: EntryNumber::new(0).expect("zero is an entry number"),
        },
        from: path(from),
        path: path(to),
    }
}

/// A batch whose additions would leave one Entry Path both an Entry and a
/// folder of Entries is refused before anything is written, naming that path;
/// the same additions commit once the batch removes or renames the other
/// Entry away (spec: EP-4, EP-6).
///
/// Both directions are asked: an addition under a path an Entry stands at,
/// where the folder above is the path named, and an addition at a path Entries
/// stand under, where the addition's own path is. So are two additions of one
/// batch against each other. The removals and renames of the batch apply
/// first, as they do for the path-uniqueness check, so a batch that takes the
/// other Entry away leaves nothing for the addition to meet.
pub async fn an_addition_that_makes_a_path_both_a_file_and_a_folder_is_refused(
    fixture: &CommitUnderTest,
) {
    let store = fixture.store();
    let keys = control_keys();
    commit_each(fixture, 1, &["docs/notes", "albums/a.jpg"]).await;
    for seed in 10..=16 {
        Library::upload_container(store, container_id(seed)).await;
    }

    let under = PreparedBatch::adding(vec![prepared(
        10,
        ContainerKind::OneFile,
        &["docs/notes/x.pdf"],
    )]);
    assert_eq!(
        refused_as_both(fixture, under).await,
        "docs/notes",
        "an addition under a path an Entry stands at names that path",
    );

    let above = PreparedBatch::adding(vec![prepared(11, ContainerKind::OneFile, &["albums"])]);
    assert_eq!(
        refused_as_both(fixture, above).await,
        "albums",
        "an addition at a path Entries stand under names its own path",
    );

    let together = PreparedBatch::adding(vec![
        prepared(12, ContainerKind::OneFile, &["books"]),
        prepared(13, ContainerKind::OneFile, &["books/b.png"]),
    ]);
    assert_eq!(
        refused_as_both(fixture, together).await,
        "books",
        "two additions of one batch are held to the rule against each other",
    );

    let removed = PreparedBatch::adding(vec![prepared(
        14,
        ContainerKind::OneFile,
        &["docs/notes/x.pdf"],
    )])
    .removing(vec![container_id(1)]);
    commit_batch(request(store, fixture.index(), &keys, removed))
        .await
        .expect("an addition under a path the batch removes the Entry from commits");

    let renamed = PreparedBatch::adding(vec![prepared(15, ContainerKind::OneFile, &["albums"])])
        .renaming(vec![renaming(2, "albums/a.jpg", "photos/a.jpg")]);
    commit_batch(request(store, fixture.index(), &keys, renamed))
        .await
        .expect("an addition at a path the batch renames the Entries away from commits");

    let index = fixture.index();
    for (one, seed) in [
        ("docs/notes/x.pdf", 14),
        ("albums", 15),
        ("photos/a.jpg", 2),
    ] {
        let location = index
            .entry_at(&path(one))
            .await
            .expect("asking the Index for a path must succeed")
            .unwrap_or_else(|| panic!("an Entry stands at {one}"));
        assert_eq!(location.container_id, container_id(seed), "{one}");
    }
    for gone in ["docs/notes", "albums/a.jpg", "books", "books/b.png"] {
        assert!(
            index
                .entry_at(&path(gone))
                .await
                .expect("asking the Index for a path must succeed")
                .is_none(),
            "nothing stands at {gone}",
        );
    }
}

/// Of two writers adding `a` and `a/b` at once, the one that loses the commit
/// race is refused once it has rebased onto the winner's head, and the
/// winner's Entry is what stands (spec: CP-7, EP-7).
///
/// Neither addition meets anything on the head both were prepared against, so
/// the conflict is the rebase's to find: the loser repeats the check against
/// the new head, where the winner's `a` stands above its `a/b`, as two writes
/// to one path meet each other.
pub async fn a_rebased_addition_under_a_file_another_writer_added_is_refused(
    fixture: &CommitUnderTest,
) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();
    for seed in [1, 2] {
        Library::upload_container(store, container_id(seed)).await;
    }

    let rival = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["a"])]);
    let racing = RacingStore::letting_in(store, fixture.other(), &keys, rival);

    let batch = PreparedBatch::adding(vec![prepared(2, ContainerKind::OneFile, &["a/b"])]);
    let result = commit_batch(request(&racing, index, &keys, batch))
        .await
        .map_err(|failure| *failure.error);

    match result {
        Err(CommitError::FileAndFolder { ref path }) => assert_eq!(path.as_str(), "a"),
        other => panic!("expected the rebased addition refused, got {other:?}"),
    }
    let library = Library::read(store).await;
    assert!(
        library.holds(&ControlObjectName::head(generation(1))),
        "the winner's record is the head",
    );
    assert!(
        !library.holds(&ControlObjectName::head(generation(2))),
        "the refused batch wrote no record of its own (spec: CP-1)",
    );
    assert!(
        index
            .entry_at(&path("a"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "the winner's file stands",
    );
    assert!(
        index
            .entry_at(&path("a/b"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_none(),
        "and nothing under it",
    );
}
