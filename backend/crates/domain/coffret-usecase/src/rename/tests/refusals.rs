//! Renames refused before anything is written (spec: FM-15, EP-4, EP-5,
//! CP-1).

use super::*;

/// What a refusal case expects, by the path it names where it names one.
enum Expected {
    NothingAt(&'static str),
    SamePath,
    ReservedNamespace(&'static str),
    IntoItself,
    DestinationTaken(&'static str),
    FileAndFolder(&'static str),
}

impl Expected {
    fn matches(&self, refusal: &RenameRefusal) -> bool {
        match (self, refusal) {
            (Self::NothingAt(expected), RenameRefusal::NothingAt { path })
            | (Self::ReservedNamespace(expected), RenameRefusal::ReservedNamespace { path })
            | (Self::DestinationTaken(expected), RenameRefusal::DestinationTaken { path })
            | (Self::FileAndFolder(expected), RenameRefusal::FileAndFolder { path }) => {
                path.as_str() == *expected
            }
            (Self::SamePath, RenameRefusal::SamePath)
            | (Self::IntoItself, RenameRefusal::IntoItself) => true,
            _ => false,
        }
    }
}

// Each refusal names what it refused, and leaves Storage exactly as it was: no
// object written, trashed or read, and the head where it stood (spec: CP-1).
#[tokio::test]
async fn each_refusal_leaves_storage_untouched() {
    let fixture = fixture().await;
    let keys = keys();
    for relative in ["albums/a.jpg", "albums/b.jpg", "notes", "docs/x.txt"] {
        write(
            fixture.fs(),
            fixture.source_folder(),
            relative,
            relative.as_bytes(),
        );
    }
    sync_source(&fixture, &keys, 1).await;
    let base = head(fixture.source()).await;

    for (from, to, expected) in [
        (
            "missing/x.jpg",
            "y.jpg",
            Expected::NothingAt("missing/x.jpg"),
        ),
        // A path that only starts with the same letters is not under it.
        ("album", "y", Expected::NothingAt("album")),
        ("albums/a.jpg", "albums/a.jpg", Expected::SamePath),
        (
            "albums/a.jpg",
            "albums/b.jpg",
            Expected::DestinationTaken("albums/b.jpg"),
        ),
        // A file stands where the destination needs a folder.
        (
            "albums/a.jpg",
            "notes/a.jpg",
            Expected::FileAndFolder("notes"),
        ),
        // Entries stand under the destination, which would then be a file.
        ("albums/a.jpg", "docs", Expected::FileAndFolder("docs")),
        ("albums", "albums/old", Expected::IntoItself),
        (
            "albums/a.jpg",
            ".coffret-derived/a.jpg",
            Expected::ReservedNamespace(".coffret-derived/a.jpg"),
        ),
        (
            ".coffret-derived/thumb",
            "thumb",
            Expected::ReservedNamespace(".coffret-derived/thumb"),
        ),
    ] {
        let store = RecordingStore::around(fixture.store());
        let result =
            rename_entries(request(&store, fixture.source(), &fixture, &keys, from, to)).await;
        match &result {
            Err(RenameError::Refused(refusal)) => assert!(
                expected.matches(refusal),
                "renaming {from} to {to} was refused as {refusal:?}",
            ),
            other => panic!("renaming {from} to {to} must be refused, got {other:?}"),
        }
        assert_eq!(store.writes(), 0, "renaming {from} to {to} wrote nothing");
        assert_eq!(store.containers_read(), Vec::<String>::new());
        assert_eq!(head(fixture.source()).await, base);
    }

    // And the files are where they were.
    for relative in ["albums/a.jpg", "albums/b.jpg", "notes", "docs/x.txt"] {
        assert!(holds(fixture.source(), relative).await);
        assert_eq!(
            file(&fixture, fixture.source_folder(), relative).as_deref(),
            Some(relative.as_bytes()),
        );
    }
}

// A folder renamed to a path above it is no folder renamed under itself: the
// subtree is renamed up a level wherever nothing clashes.
#[tokio::test]
async fn a_folder_may_be_renamed_to_a_path_above_it() {
    let fixture = fixture().await;
    let keys = keys();
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/2026/a.jpg",
        b"a",
    );
    sync_source(&fixture, &keys, 1).await;

    let outcome = rename_entries(request(
        fixture.store(),
        fixture.source(),
        &fixture,
        &keys,
        "albums/2026",
        "2026",
    ))
    .await
    .unwrap_or_else(|error| panic!("the rename must commit: {error}"));
    assert_eq!(
        outcome.renamed,
        [renamed("albums/2026/a.jpg", "2026/a.jpg")]
    );
}
