//! Renames that commit: one Entry, a folder of them across Containers, and a
//! rename back.

use super::*;

const COVER: &[u8] = b"a cover kept in a Container of its own";
const VOL1: &[u8] = b"the first volume";
const VOL2: &[u8] = b"the second volume, a little longer";
const VOL3: &[u8] = b"the third volume, synced on its own";
const NOTES: &[u8] = b"notes packed beside the books and left where they are";

// FM-15, CP-1, EP-10: renaming one Entry commits one record whose only change
// is that rename, names it by its Container and entry number, writes no
// Container and reads none — and the renaming device's own file follows it at
// once.
#[tokio::test]
async fn a_file_is_renamed_in_one_record_without_uploading_anything() {
    let fixture = fixture().await;
    let keys = keys();
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/cover.jpg",
        COVER,
    );
    sync_source(&fixture, &keys, 1).await;
    let before = entry_at(fixture.source(), "albums/cover.jpg").await;
    let base = head(fixture.source()).await;

    let store = RecordingStore::around(fixture.store());
    let outcome = rename_entries(request(
        &store,
        fixture.source(),
        &fixture,
        &keys,
        "albums/cover.jpg",
        "albums/front.jpg",
    ))
    .await
    .unwrap_or_else(|error| panic!("the rename must commit: {error}"));

    assert!(!outcome.folder);
    assert_eq!(
        outcome.renamed,
        [renamed("albums/cover.jpg", "albums/front.jpg")]
    );
    let record = &outcome.commit.record;
    assert_eq!(record.generation(), base.next().expect("a successor"));
    assert!(record.additions().is_empty(), "nothing is added");
    assert!(record.removals().is_empty(), "nothing is removed");
    assert_eq!(
        record.renames(),
        [EntryRename::new(
            before.container_id,
            before.entry_number,
            entry_path("albums/front.jpg")
        )
        .expect("an ordinary path")],
    );
    assert_eq!(store.containers_put(), Vec::<String>::new());
    assert_eq!(store.containers_read(), Vec::<String>::new());

    let after = entry_at(fixture.source(), "albums/front.jpg").await;
    assert_eq!(
        (after.container_id, after.entry_number),
        (before.container_id, before.entry_number),
        "the same Entry of the same Container",
    );
    assert_eq!(
        after.entry.captured_path.as_str(),
        "albums/cover.jpg",
        "its Container still calls it what it was captured as",
    );
    assert!(!holds(fixture.source(), "albums/cover.jpg").await);

    assert_eq!(
        relocated(&outcome.relocations),
        [(
            entry_path("albums/cover.jpg"),
            entry_path("albums/front.jpg")
        )],
    );
    assert_eq!(
        file(&fixture, fixture.source_folder(), "albums/front.jpg").as_deref(),
        Some(COVER),
    );
    assert_eq!(
        file(&fixture, fixture.source_folder(), "albums/cover.jpg"),
        None
    );
}

// FM-15, EP-7, CP-1: a folder is renamed Entry by Entry, each enumerated in the
// record with the rest of its path kept, across a Pack that also holds an
// Entry outside the folder and a one-file Container — and still nothing is
// uploaded or read, and the Entry outside the folder stays where it is.
#[tokio::test]
async fn a_folder_spanning_several_containers_is_renamed_entry_by_entry() {
    let fixture = fixture().await;
    let keys = keys();
    write(
        fixture.fs(),
        fixture.source_folder(),
        "books/vol1.txt",
        VOL1,
    );
    write(
        fixture.fs(),
        fixture.source_folder(),
        "books/sub/vol2.txt",
        VOL2,
    );
    write(fixture.fs(), fixture.source_folder(), "notes.txt", NOTES);
    let frozen = freeze_source(&fixture, &keys, ONE_PACK, 1).await;
    assert_eq!(frozen.packs.len(), 1, "the three files share one Pack");
    let pack = frozen.packs[0].container_id;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "books/sub/vol3.txt",
        VOL3,
    );
    sync_source(&fixture, &keys, 2).await;
    let vol3 = entry_at(fixture.source(), "books/sub/vol3.txt").await;
    assert_ne!(vol3.container_id, pack, "a Container of its own");

    let store = RecordingStore::around(fixture.store());
    let outcome = rename_entries(request(
        &store,
        fixture.source(),
        &fixture,
        &keys,
        "books",
        "shelf/books",
    ))
    .await
    .unwrap_or_else(|error| panic!("the rename must commit: {error}"));

    assert!(outcome.folder);
    assert_eq!(
        outcome.renamed,
        [
            renamed("books/sub/vol2.txt", "shelf/books/sub/vol2.txt"),
            renamed("books/sub/vol3.txt", "shelf/books/sub/vol3.txt"),
            renamed("books/vol1.txt", "shelf/books/vol1.txt"),
        ],
    );
    let record = &outcome.commit.record;
    assert_eq!(record.renames().len(), 3, "one element per Entry");
    assert!(record.additions().is_empty() && record.removals().is_empty());
    let mut paths = record_paths(record.renames());
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "shelf/books/sub/vol2.txt",
            "shelf/books/sub/vol3.txt",
            "shelf/books/vol1.txt"
        ],
    );
    assert_eq!(store.containers_put(), Vec::<String>::new());
    assert_eq!(store.containers_read(), Vec::<String>::new());

    for (path, container) in [
        ("shelf/books/vol1.txt", pack),
        ("shelf/books/sub/vol2.txt", pack),
        ("shelf/books/sub/vol3.txt", vol3.container_id),
        ("notes.txt", pack),
    ] {
        assert_eq!(
            entry_at(fixture.source(), path).await.container_id,
            container,
            "{path} is in the Container it was in",
        );
    }
    for gone in ["books/vol1.txt", "books/sub/vol2.txt", "books/sub/vol3.txt"] {
        assert!(!holds(fixture.source(), gone).await, "{gone} is gone");
    }

    assert_eq!(relocated(&outcome.relocations).len(), 3);
    for (relative, content) in [
        ("shelf/books/vol1.txt", VOL1),
        ("shelf/books/sub/vol2.txt", VOL2),
        ("shelf/books/sub/vol3.txt", VOL3),
        ("notes.txt", NOTES),
    ] {
        assert_eq!(
            file(&fixture, fixture.source_folder(), relative).as_deref(),
            Some(content),
            "{relative} on the renaming device",
        );
    }
    assert_eq!(
        file(&fixture, fixture.source_folder(), "books/vol1.txt"),
        None
    );
}

// FM-15: renaming an Entry back to the name its Container captured it under is
// an ordinary rename, and leaves the catalog holding no captured path apart
// from the current one.
#[tokio::test]
async fn a_rename_back_returns_to_the_captured_name() {
    let fixture = fixture().await;
    let keys = keys();
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/cover.jpg",
        COVER,
    );
    sync_source(&fixture, &keys, 1).await;

    for (from, to) in [
        ("albums/cover.jpg", "albums/front.jpg"),
        ("albums/front.jpg", "albums/cover.jpg"),
    ] {
        rename_entries(request(
            fixture.store(),
            fixture.source(),
            &fixture,
            &keys,
            from,
            to,
        ))
        .await
        .unwrap_or_else(|error| panic!("renaming {from} to {to} must commit: {error}"));
    }

    let back = entry_at(fixture.source(), "albums/cover.jpg").await;
    assert_eq!(back.entry.captured_path, back.entry.path);
    assert!(!holds(fixture.source(), "albums/front.jpg").await);
    assert_eq!(
        file(&fixture, fixture.source_folder(), "albums/cover.jpg").as_deref(),
        Some(COVER),
    );
}

// EP-10, EP-11: a device that fetched the files before the rename catches the
// rename up on its next fetch and relocates its own files to the new paths,
// reading no Container for content it already holds.
#[tokio::test]
async fn another_device_relocates_its_files_without_reading_a_container() {
    let fixture = fixture().await;
    let keys = keys();
    write(
        fixture.fs(),
        fixture.source_folder(),
        "books/vol1.txt",
        VOL1,
    );
    write(
        fixture.fs(),
        fixture.source_folder(),
        "books/sub/vol2.txt",
        VOL2,
    );
    freeze_source(&fixture, &keys, ONE_PACK, 1).await;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/cover.jpg",
        COVER,
    );
    sync_source(&fixture, &keys, 2).await;
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;
    let first = RecordingStore::around(fixture.store());
    fetch_folders(fetch_request(&first, &fixture, &keys, 3))
        .await
        .unwrap_or_else(|error| panic!("the first fetch must succeed: {error}"));
    assert_eq!(
        first.containers_read().len(),
        2,
        "the first fetch reads the Pack and the one-file Container, which is what \
         the count below would see again",
    );

    for (from, to) in [("books", "shelf"), ("albums/cover.jpg", "albums/front.jpg")] {
        rename_entries(request(
            fixture.store(),
            fixture.source(),
            &fixture,
            &keys,
            from,
            to,
        ))
        .await
        .unwrap_or_else(|error| panic!("renaming {from} must commit: {error}"));
    }

    let store = RecordingStore::around(fixture.store());
    let outcome = fetch_folders(fetch_request(&store, &fixture, &keys, 5))
        .await
        .unwrap_or_else(|error| panic!("the fetch after the rename must succeed: {error}"));

    assert_eq!(store.containers_read(), Vec::<String>::new());
    assert_eq!(store.writes(), 0, "a fetch writes nothing to Storage");
    assert!(outcome.fetched.is_empty(), "{:?}", outcome.fetched);
    assert_eq!(relocated(&outcome.relocations).len(), 3);
    for (relative, content) in [
        ("shelf/vol1.txt", VOL1),
        ("shelf/sub/vol2.txt", VOL2),
        ("albums/front.jpg", COVER),
    ] {
        assert_eq!(
            file(&fixture, fixture.target_folder(), relative).as_deref(),
            Some(content),
            "{relative} on the other device",
        );
    }
    for gone in ["books/vol1.txt", "books/sub/vol2.txt", "albums/cover.jpg"] {
        assert_eq!(file(&fixture, fixture.target_folder(), gone), None);
    }
}
