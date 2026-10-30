//! A rename of a set of pairs: committed in one record, or refused as a whole
//! (spec: FM-15, CP-1, EP-18).

use super::*;
use crate::rename::{preview_rename, RenamePair};

/// The request renaming every `(from, to)` of `pairs` at once through the
/// source device, relocating its files after the commit.
fn set_request<'a>(
    store: &'a dyn ObjectStore,
    fixture: &'a FetchUnderTest,
    keys: &'a LibraryKeys,
    pairs: &[(&str, &str)],
) -> RenameRequest<'a> {
    RenameRequest::of_pairs(
        store,
        fixture.source(),
        keys,
        fixture.fs(),
        pairs
            .iter()
            .map(|(from, to)| RenamePair::new(entry_path(*from), entry_path(*to)))
            .collect(),
        renamed_at(),
    )
    .with_policy(policy())
    .with_local_disk(fixture.fs(), fixture.fs(), fixture.fs())
}

/// A source device whose folder holds a file, and a folder of two, carried in.
async fn three_files(fixture: &FetchUnderTest, keys: &LibraryKeys) {
    for relative in ["cover.jpg", "albums/a.jpg", "albums/b.jpg"] {
        write(
            fixture.fs(),
            fixture.source_folder(),
            relative,
            relative.as_bytes(),
        );
    }
    sync_source(fixture, keys, 1).await;
}

// EP-18, FM-15, CP-1, EP-10: a file and a folder moved at once land in one
// record, which renames every Entry they name, and the device's files follow.
#[tokio::test]
async fn several_pairs_are_committed_in_one_record() {
    let fixture = fixture().await;
    let keys = keys();
    three_files(&fixture, &keys).await;
    let base = head(fixture.source()).await;

    let store = RecordingStore::around(fixture.store());
    let outcome = rename_entries(set_request(
        &store,
        &fixture,
        &keys,
        &[("cover.jpg", "kept/cover.jpg"), ("albums", "kept/albums")],
    ))
    .await
    .unwrap_or_else(|error| panic!("the set must commit: {error}"));

    assert!(outcome.folder, "one of the pairs named a folder");
    assert_eq!(
        outcome.renamed,
        [
            renamed("albums/a.jpg", "kept/albums/a.jpg"),
            renamed("albums/b.jpg", "kept/albums/b.jpg"),
            renamed("cover.jpg", "kept/cover.jpg"),
        ]
    );
    let record = &outcome.commit.record;
    assert_eq!(
        record.generation(),
        base.next().expect("a successor"),
        "one record for the whole set"
    );
    let mut paths = record_paths(record.renames());
    paths.sort_unstable();
    assert_eq!(
        paths,
        ["kept/albums/a.jpg", "kept/albums/b.jpg", "kept/cover.jpg"]
    );
    assert_eq!(store.containers_put(), Vec::<String>::new());
    assert_eq!(
        relocated(&outcome.relocations),
        [
            (entry_path("albums/a.jpg"), entry_path("kept/albums/a.jpg")),
            (entry_path("albums/b.jpg"), entry_path("kept/albums/b.jpg")),
            (entry_path("cover.jpg"), entry_path("kept/cover.jpg")),
        ]
    );
    assert_eq!(
        file(&fixture, fixture.source_folder(), "kept/albums/a.jpg").as_deref(),
        Some(&b"albums/a.jpg"[..])
    );
}

// EP-18, FM-15: a folder and a file under it named in one request are one
// move of the folder — the file keeps its place inside it, whatever its own
// pair said.
#[tokio::test]
async fn a_file_named_beside_its_folder_moves_with_the_folder() {
    let fixture = fixture().await;
    let keys = keys();
    three_files(&fixture, &keys).await;

    let outcome = rename_entries(set_request(
        fixture.store(),
        &fixture,
        &keys,
        &[("albums", "kept/albums"), ("albums/a.jpg", "kept/a.jpg")],
    ))
    .await
    .unwrap_or_else(|error| panic!("the set must commit: {error}"));

    assert_eq!(
        outcome.renamed,
        [
            renamed("albums/a.jpg", "kept/albums/a.jpg"),
            renamed("albums/b.jpg", "kept/albums/b.jpg"),
        ]
    );
}

/// Whether a refusal is the one a case expects.
type Expected = dyn Fn(&RenameRefusal) -> bool;

// EP-18, FM-15, CP-1: one pair the Library refuses, or two pairs that overlap,
// refuse the whole set before anything is written, and every Entry stays
// where it was.
#[tokio::test]
async fn one_refused_or_overlapping_pair_refuses_the_whole_set() {
    let fixture = fixture().await;
    let keys = keys();
    three_files(&fixture, &keys).await;
    let base = head(fixture.source()).await;

    let overlapping =
        |refusal: &RenameRefusal| matches!(refusal, RenameRefusal::Overlapping { .. });
    let nothing_at = |refusal: &RenameRefusal| matches!(refusal, RenameRefusal::NothingAt { .. });
    let cases: [(&[(&str, &str)], &Expected); 5] = [
        // The second pair names nothing the Library holds.
        (
            &[("cover.jpg", "front.jpg"), ("missing.jpg", "found.jpg")],
            &nothing_at,
        ),
        // Two pairs with one destination.
        (
            &[("cover.jpg", "kept/x"), ("albums/a.jpg", "kept/x")],
            &overlapping,
        ),
        // A destination under another pair's destination.
        (
            &[("cover.jpg", "kept"), ("albums", "kept/albums")],
            &overlapping,
        ),
        // One source sent two places.
        (
            &[("cover.jpg", "one.jpg"), ("cover.jpg", "two.jpg")],
            &overlapping,
        ),
        // No pair at all.
        (&[], &|refusal| matches!(refusal, RenameRefusal::Nothing)),
    ];
    for (pairs, expected) in cases {
        let store = RecordingStore::around(fixture.store());
        let result = rename_entries(set_request(&store, &fixture, &keys, pairs)).await;
        match &result {
            Err(RenameError::Refused(refusal)) => {
                assert!(expected(refusal), "{pairs:?} was refused as {refusal:?}");
            }
            other => panic!("{pairs:?} must be refused, got {other:?}"),
        }
        assert_eq!(store.writes(), 0, "{pairs:?} wrote nothing");
        assert_eq!(head(fixture.source()).await, base);
    }
    for relative in ["cover.jpg", "albums/a.jpg", "albums/b.jpg"] {
        assert!(holds(fixture.source(), relative).await, "{relative} stays");
        assert_eq!(
            file(&fixture, fixture.source_folder(), relative).as_deref(),
            Some(relative.as_bytes()),
        );
    }
}

// FM-15, EP-10, EP-15: a preview of the same set writes nothing, counts the
// Entries the run would rename and this device's files that would follow, and
// names a file edited since it was placed.
#[tokio::test]
async fn a_preview_counts_what_the_set_would_move_and_writes_nothing() {
    let fixture = fixture().await;
    let keys = keys();
    three_files(&fixture, &keys).await;
    let base = head(fixture.source()).await;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/b.jpg",
        b"edited since this device placed it",
    );
    touch(
        fixture.fs(),
        &fixture.source_folder().join("albums/b.jpg"),
        NEWER,
    );

    let store = RecordingStore::around(fixture.store());
    let preview = preview_rename(set_request(
        &store,
        &fixture,
        &keys,
        &[("cover.jpg", "kept/cover.jpg"), ("albums", "kept/albums")],
    ))
    .await
    .unwrap_or_else(|error| panic!("the preview must answer: {error}"));

    assert!(preview.refused.is_none(), "{:?}", preview.refused);
    assert_eq!(preview.renamed.len(), 3);
    assert_eq!(preview.following, 2, "the two unedited files follow");
    assert_eq!(preview.leaving, 0);
    assert_eq!(preview.edited, [entry_path("albums/b.jpg")]);
    assert_eq!(store.writes(), 0, "a preview writes nothing");
    assert_eq!(head(fixture.source()).await, base);
    assert!(holds(fixture.source(), "cover.jpg").await);
    assert_eq!(
        file(&fixture, fixture.source_folder(), "cover.jpg").as_deref(),
        Some(&b"cover.jpg"[..])
    );

    let refused = preview_rename(set_request(
        &store,
        &fixture,
        &keys,
        &[("cover.jpg", "kept"), ("albums", "kept/albums")],
    ))
    .await
    .unwrap_or_else(|error| panic!("the preview must answer: {error}"));
    assert!(
        matches!(refused.refused, Some(RenameRefusal::Overlapping { .. })),
        "{:?}",
        refused.refused
    );
    assert!(refused.renamed.is_empty());
}
