//! A file that follows its Entry across two mapped roots on one filesystem,
//! and one that cannot follow and leaves this device (spec: EP-10, EP-15).

use std::path::{Path, PathBuf};

use crate::entry_paths::entry_path;
use crate::relocate::{Relocation, RelocationRefused};
use crate::sync::SyncOutcome;
use crate::sync_conformance::fixtures::{another_filesystem, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

use super::{
    at, carry_in, map_registered, refused, relocated, rename, row, sync, uploaded_nothing,
};

/// The bytes of the one file each case carries in.
const BYTES: &[u8] = b"the file's bytes";

/// Two roots mapped for `albums` and `books`, the first holding `a.jpg`,
/// carried in by a first sync that stamps both roots (spec: EP-12).
async fn two_roots(fixture: &SyncUnderTest) -> (PathBuf, PathBuf) {
    let albums = fixture.folder().join("albums-root");
    let books = fixture.folder().join("books-root");
    fixture.fs().create_dir(&albums);
    fixture.fs().create_dir(&books);
    map_registered(fixture, Some("albums"), &albums).await;
    map_registered(fixture, Some("books"), &books).await;
    write(fixture.fs(), &albums, "a.jpg", BYTES);
    assert_eq!(sync(fixture, 1).await.added.len(), 1);
    (albums, books)
}

/// The one file that left this device in a run, as `(from, to)`; panics on any other outcome.
fn left(outcome: &SyncOutcome) -> (String, String) {
    match outcome.relocations.as_slice() {
        [Relocation::Left { from, to }] => (from.as_str().to_owned(), to.as_str().to_owned()),
        other => panic!("expected one file to leave this device, got {other:?}"),
    }
}

/// Asserts that the file at `file` went to the trash and that no record of
/// it is kept at `from` or at `to`: the Entry is in the Library, and not on
/// this device, at its new path.
async fn gone_to_the_trash(fixture: &SyncUnderTest, file: &Path, from: &str, to: &str) {
    assert!(!fixture.fs().holds(file), "the file leaves {from}");
    assert_eq!(fixture.fs().discarded(), [file.to_path_buf()]);
    assert!(
        row(fixture, from).await.is_none(),
        "its record is forgotten"
    );
    assert!(row(fixture, to).await.is_none(), "and none is made at {to}");
    assert!(
        fixture
            .index()
            .entry_at(&entry_path(to))
            .await
            .expect("looking a path up must succeed")
            .is_some(),
        "the Entry stands at {to}"
    );
}

/// A rename that carries an Entry from under one mapped root to under another
/// on the same filesystem is followed with one rename, as within one root
/// (spec: EP-10, EP-12).
pub async fn a_file_follows_its_entry_across_mapped_roots_on_one_filesystem(
    fixture: &SyncUnderTest,
) {
    let (albums, books) = two_roots(fixture).await;
    rename(fixture, &[("albums/a.jpg", "books/vol-1/a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert_eq!(
        relocated(&outcome),
        [("albums/a.jpg".to_owned(), "books/vol-1/a.jpg".to_owned())]
    );
    assert!(!fixture.fs().holds(&albums.join("a.jpg")));
    assert_eq!(
        fixture.fs().content(&books.join("vol-1/a.jpg")).as_deref(),
        Some(BYTES)
    );
    let record = row(fixture, "books/vol-1/a.jpg")
        .await
        .expect("the record follows the file");
    assert!(!record.awaits_relocation());
    assert!(fixture.fs().discarded().is_empty(), "nothing is discarded");
}

/// A rename to a path no mapped folder of this device reaches leaves the
/// unedited file nowhere to go: it is discarded into the trash and its record
/// forgotten, and the run reports it (spec: EP-10, EP-15).
pub async fn a_file_whose_new_path_no_mapping_reaches_leaves_the_device(fixture: &SyncUnderTest) {
    let (albums, _) = two_roots(fixture).await;
    rename(fixture, &[("albums/a.jpg", "scans/a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert_eq!(
        left(&outcome),
        ("albums/a.jpg".to_owned(), "scans/a.jpg".to_owned())
    );
    gone_to_the_trash(
        fixture,
        &albums.join("a.jpg"),
        "albums/a.jpg",
        "scans/a.jpg",
    )
    .await;
    let next = sync(fixture, 3).await;
    assert!(next.relocations.is_empty(), "nothing is left waiting");
}

/// A rename into a mapped root the scans recorded on another filesystem is
/// not tried as a rename: the unedited file leaves this device, discarded into the trash
/// (spec: EP-10, EP-12).
pub async fn a_file_whose_new_root_is_on_another_filesystem_leaves_the_device(
    fixture: &SyncUnderTest,
) {
    let albums = fixture.folder().join("albums-root");
    let books = fixture.folder().join("books-root");
    fixture.fs().create_dir(&albums);
    fixture.fs().create_dir(&books);
    fixture.fs().set_root_identity(&books, another_filesystem());
    map_registered(fixture, Some("albums"), &albums).await;
    map_registered(fixture, Some("books"), &books).await;
    write(fixture.fs(), &albums, "a.jpg", BYTES);
    write(fixture.fs(), &books, "b.jpg", BYTES);
    assert_eq!(sync(fixture, 1).await.added.len(), 2);
    rename(fixture, &[("albums/a.jpg", "books/a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert_eq!(
        left(&outcome),
        ("albums/a.jpg".to_owned(), "books/a.jpg".to_owned())
    );
    gone_to_the_trash(
        fixture,
        &albums.join("a.jpg"),
        "albums/a.jpg",
        "books/a.jpg",
    )
    .await;
    assert!(
        !fixture.fs().holds(&books.join("a.jpg")),
        "nothing is copied"
    );
}

/// A rename the filesystem refuses as crossing onto another one — a disk
/// mounted below the mapped root, which no scan tells apart — is a file that
/// cannot follow, and the unedited file leaves this device, discarded into the trash
/// (spec: EP-10).
pub async fn a_file_the_filesystem_will_not_rename_across_leaves_the_device(
    fixture: &SyncUnderTest,
) {
    carry_in(fixture, &[("a.jpg", BYTES)]).await;
    let mounted = at(fixture, "mounted");
    fixture.fs().mount(&mounted);
    rename(fixture, &[("a.jpg", "mounted/a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert_eq!(
        left(&outcome),
        ("a.jpg".to_owned(), "mounted/a.jpg".to_owned())
    );
    gone_to_the_trash(fixture, &at(fixture, "a.jpg"), "a.jpg", "mounted/a.jpg").await;
    assert!(
        !fixture.fs().holds(&mounted.join("a.jpg")),
        "nothing is copied"
    );
}

/// An edited file that cannot follow its Entry is never discarded: it stays
/// where it stands, its record waiting, and every run reports it for its edit
/// (spec: EP-10, EP-15).
pub async fn an_edited_file_that_cannot_follow_is_kept_and_reported(fixture: &SyncUnderTest) {
    let (albums, _) = two_roots(fixture).await;
    rename(fixture, &[("albums/a.jpg", "scans/a.jpg")]).await;
    let file = albums.join("a.jpg");
    fixture.fs().write_file(&file, b"edited since");

    for run in [2, 3] {
        let outcome = sync(fixture, run).await;

        uploaded_nothing(&outcome);
        let (from, to, reason) = refused(&outcome);
        assert_eq!(
            (from.as_str(), to.as_str()),
            ("albums/a.jpg", "scans/a.jpg")
        );
        assert!(matches!(reason, RelocationRefused::Edited), "{reason:?}");
        assert_eq!(
            fixture.fs().content(&file).as_deref(),
            Some(&b"edited since"[..])
        );
        assert!(fixture.fs().discarded().is_empty(), "nothing is discarded");
        assert_eq!(
            row(fixture, "albums/a.jpg")
                .await
                .expect("the record stays")
                .relocate_to,
            Some(entry_path("scans/a.jpg"))
        );
    }
}
