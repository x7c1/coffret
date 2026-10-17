use crate::entry_paths::entry_path;
use crate::local_operation::LocalOperation;
use crate::relocate::{Relocation, RelocationRefused};
use crate::sync::SyncOutcome;
use crate::sync_conformance::fixtures::another_filesystem;
use crate::sync_conformance::sync_under_test::SyncUnderTest;
use crate::unavailable_root::RootUnavailable;

use super::{
    at, carry_in, content, map_registered, refused, relocated, rename, row, sync, uploaded_nothing,
};
use crate::sync_conformance::fixtures::write;

/// The bytes of the one file most cases carry in.
const BYTES: &[u8] = b"the file's bytes";

/// Asserts that a refused relocation left the file and its record exactly
/// where they were: the file at `from` with its bytes, still waiting for `to`,
/// and nothing claimed at `to`.
async fn left_waiting(fixture: &SyncUnderTest, from: &str, to: &str) {
    assert_eq!(
        content(fixture, from).as_deref(),
        Some(BYTES),
        "{from} stays"
    );
    let waiting = row(fixture, from).await.expect("the record stays");
    assert_eq!(waiting.relocate_to, Some(entry_path(to)), "and still waits");
    let standing = row(fixture, to).await;
    assert!(
        standing.is_none(),
        "nothing is claimed at {to}: {standing:?}"
    );
}

/// A root standing on another filesystem than the one its mapping records is
/// not relocated in: the file waits, and is reported. The scan that finds the
/// root holding files re-stamps it, and the next run relocates (spec: EP-10,
/// EP-12).
pub async fn a_relocation_under_an_unavailable_root_waits(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", BYTES)]).await;
    rename(fixture, &[("a.jpg", "b.jpg")]).await;
    fixture
        .fs()
        .set_root_identity(fixture.folder(), another_filesystem());

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    let (from, to, reason) = refused(&outcome);
    assert_eq!((from.as_str(), to.as_str()), ("a.jpg", "b.jpg"));
    assert!(
        matches!(
            reason,
            RelocationRefused::Unavailable(RootUnavailable::AnotherFilesystem)
        ),
        "{reason:?}"
    );
    left_waiting(fixture, "a.jpg", "b.jpg").await;

    let next = sync(fixture, 3).await;
    assert_eq!(relocated(&next), [("a.jpg".to_owned(), "b.jpg".to_owned())]);
    assert_eq!(content(fixture, "b.jpg").as_deref(), Some(BYTES));
}

/// A file edited since this device placed it is not the copy its record is
/// about, so it is not relocated: it stays, reported, and is relocated once it
/// holds what the record says again (spec: EP-10, EP-15).
pub async fn a_file_edited_since_it_was_placed_is_not_relocated(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", BYTES)]).await;
    rename(fixture, &[("a.jpg", "b.jpg")]).await;
    let file = at(fixture, "a.jpg");
    fixture.fs().write_file(&file, b"edited since");

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    let (_, _, reason) = refused(&outcome);
    assert!(matches!(reason, RelocationRefused::Edited), "{reason:?}");
    assert_eq!(
        content(fixture, "a.jpg").as_deref(),
        Some(&b"edited since"[..])
    );
    assert!(!fixture.fs().holds(&at(fixture, "b.jpg")));
    assert_eq!(
        row(fixture, "a.jpg")
            .await
            .expect("the record stays")
            .relocate_to,
        Some(entry_path("b.jpg"))
    );

    fixture.fs().write_file(&file, BYTES);
    let next = sync(fixture, 3).await;
    assert_eq!(relocated(&next), [("a.jpg".to_owned(), "b.jpg".to_owned())]);
}

/// Something this device did not place standing at the new path is never
/// overwritten: the file waits, reported, and is relocated once the path is free
/// (spec: EP-10, EP-11).
pub async fn something_standing_at_the_new_path_is_never_overwritten(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", BYTES)]).await;
    rename(fixture, &[("a.jpg", "b.jpg")]).await;
    let theirs = write(fixture.fs(), fixture.folder(), "b.jpg", b"someone else's");

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    let (_, _, reason) = refused(&outcome);
    assert!(
        matches!(reason, RelocationRefused::DestinationTaken),
        "{reason:?}"
    );
    assert_eq!(
        content(fixture, "b.jpg").as_deref(),
        Some(&b"someone else's"[..])
    );
    left_waiting(fixture, "a.jpg", "b.jpg").await;

    fixture.fs().remove_file(&theirs);
    let next = sync(fixture, 3).await;
    assert_eq!(relocated(&next), [("a.jpg".to_owned(), "b.jpg".to_owned())]);
    assert_eq!(content(fixture, "b.jpg").as_deref(), Some(BYTES));
}

/// A rename that carries an Entry from under one mapped root to under another
/// is not followed: no rename crosses two roots, and nothing is copied. A
/// later rename back under the file's own root is (spec: EP-9, EP-10).
pub async fn a_relocation_across_mapped_roots_is_refused(fixture: &SyncUnderTest) {
    let albums = fixture.folder().join("albums-root");
    let books = fixture.folder().join("books-root");
    fixture.fs().create_dir(&albums);
    fixture.fs().create_dir(&books);
    map_registered(fixture, Some("albums"), &albums).await;
    map_registered(fixture, Some("books"), &books).await;
    let file = write(fixture.fs(), &albums, "a.jpg", BYTES);
    assert_eq!(sync(fixture, 1).await.added.len(), 1);
    rename(fixture, &[("albums/a.jpg", "books/a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    let (_, _, reason) = refused(&outcome);
    assert!(
        matches!(reason, RelocationRefused::OutsideOneRoot),
        "{reason:?}"
    );
    assert!(fixture.fs().holds(&file));
    assert!(!fixture.fs().holds(&books.join("a.jpg")));

    rename(fixture, &[("books/a.jpg", "albums/c.jpg")]).await;
    let next = sync(fixture, 3).await;
    assert_eq!(
        relocated(&next),
        [("albums/a.jpg".to_owned(), "albums/c.jpg".to_owned())]
    );
    assert_eq!(
        fixture.fs().content(&albums.join("c.jpg")).as_deref(),
        Some(BYTES)
    );
}

/// A new path on another filesystem than the file — a disk mounted below the
/// mapped root — is refused rather than reached by copying and deleting, and
/// followed once both stand on one (spec: EP-10).
pub async fn a_relocation_onto_another_filesystem_is_refused(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", BYTES)]).await;
    let mounted = at(fixture, "mounted");
    fixture.fs().mount(&mounted);
    rename(fixture, &[("a.jpg", "mounted/a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    let (_, _, reason) = refused(&outcome);
    assert!(
        matches!(reason, RelocationRefused::AnotherFilesystem),
        "{reason:?}"
    );
    left_waiting(fixture, "a.jpg", "mounted/a.jpg").await;

    fixture.fs().unmount(&mounted);
    let next = sync(fixture, 3).await;
    assert_eq!(
        relocated(&next),
        [("a.jpg".to_owned(), "mounted/a.jpg".to_owned())]
    );
}

/// A file on a filesystem that offers no rename replacing nothing — a FAT
/// volume or a network share mounted below the mapped root — is not relocated
/// by checking that the new path is free and renaming onto it: the file and its
/// record wait, reported every run, nothing at the new path is made or
/// overwritten, and the file follows once it stands on a filesystem that offers
/// the rename (spec: EP-10, EP-11).
pub async fn a_relocation_without_an_atomic_rename_is_refused(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("shared/a.jpg", BYTES)]).await;
    let shared = at(fixture, "shared");
    fixture.fs().mount_without_atomic_rename(&shared);
    rename(fixture, &[("shared/a.jpg", "shared/b.jpg")]).await;

    for run in [2, 3] {
        let outcome = sync(fixture, run).await;

        uploaded_nothing(&outcome);
        let (from, to, reason) = refused(&outcome);
        assert_eq!(
            (from.as_str(), to.as_str()),
            ("shared/a.jpg", "shared/b.jpg")
        );
        assert!(
            matches!(reason, RelocationRefused::NoAtomicRename(_)),
            "run {run}: {reason:?}"
        );
        left_waiting(fixture, "shared/a.jpg", "shared/b.jpg").await;
        assert!(!fixture.fs().holds(&at(fixture, "shared/b.jpg")));
    }

    fixture.fs().unmount(&shared);
    let next = sync(fixture, 4).await;
    assert_eq!(
        relocated(&next),
        [("shared/a.jpg".to_owned(), "shared/b.jpg".to_owned())]
    );
    assert_eq!(content(fixture, "shared/b.jpg").as_deref(), Some(BYTES));
}

/// A name on the way to the new path that is not a folder of the root — the
/// fake's stand-in for a symbolic link — is never relocated through: the file
/// waits, the refusal names where the way stopped, and the file is relocated once the
/// name is a folder of the root (spec: EP-8, EP-10).
pub async fn a_relocation_through_a_symbolic_link_is_refused(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", BYTES)]).await;
    let linked = at(fixture, "linked");
    fixture.fs().plant_other(&linked);
    rename(fixture, &[("a.jpg", "linked/a.jpg")]).await;

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    let (_, _, reason) = refused(&outcome);
    match reason {
        RelocationRefused::SymbolicLink { stopped_at } => assert_eq!(stopped_at, linked),
        other => panic!("expected the link to stop the relocation, got {other:?}"),
    }
    left_waiting(fixture, "a.jpg", "linked/a.jpg").await;

    fixture.fs().remove_other(&linked);
    let next = sync(fixture, 3).await;
    assert_eq!(
        relocated(&next),
        [("a.jpg".to_owned(), "linked/a.jpg".to_owned())]
    );
}

/// Why a run refused to relocate the file at `from` to `to`; panics where it
/// reported no such refusal.
fn refusal<'o>(outcome: &'o SyncOutcome, from: &str, to: &str) -> &'o RelocationRefused {
    outcome
        .relocations
        .iter()
        .find_map(|one| match one {
            Relocation::Refused {
                from: refused_from,
                to: refused_to,
                reason,
            } if refused_from.as_str() == from && refused_to.as_str() == to => Some(reason),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "expected {from} refused for {to}, got {:?}",
                outcome.relocations
            )
        })
}

/// A file whose new path holds this device's own file, itself waiting and
/// refused this run, is reported as waiting on that one — not as a file the
/// person put there — and follows it once it is relocated (spec: EP-10).
pub async fn a_file_behind_a_refused_waiting_file_waits_for_it(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", b"a's bytes"), ("b.jpg", b"b's own")]).await;
    rename(fixture, &[("a.jpg", "b.jpg"), ("b.jpg", "c.jpg")]).await;
    let in_the_way = at(fixture, "b.jpg");
    fixture.fs().write_file(&in_the_way, b"edited since");

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert_eq!(outcome.relocations.len(), 2, "{:?}", outcome.relocations);
    let behind = refusal(&outcome, "a.jpg", "b.jpg");
    assert!(
        matches!(behind, RelocationRefused::WaitingFileInTheWay),
        "{behind:?}"
    );
    let edited = refusal(&outcome, "b.jpg", "c.jpg");
    assert!(matches!(edited, RelocationRefused::Edited), "{edited:?}");
    assert_eq!(
        content(fixture, "a.jpg").as_deref(),
        Some(&b"a's bytes"[..])
    );

    fixture.fs().write_file(&in_the_way, b"b's own");
    let next = sync(fixture, 3).await;
    assert_eq!(
        relocated(&next),
        [("a.jpg", "b.jpg"), ("b.jpg", "c.jpg")].map(|(from, to)| (from.to_owned(), to.to_owned())),
    );
    assert_eq!(
        content(fixture, "b.jpg").as_deref(),
        Some(&b"a's bytes"[..])
    );
    assert_eq!(content(fixture, "c.jpg").as_deref(), Some(&b"b's own"[..]));
}

/// A cycle whose exchange the disk refuses partway leaves the files still to
/// come where they stand, each reported as waiting on the one at its new path
/// — this device's own, not the person's — and the next run finishes the cycle
/// (spec: EP-10, FM-15).
pub async fn a_cycle_refused_partway_reports_the_rest_as_waiting(fixture: &SyncUnderTest) {
    carry_in(
        fixture,
        &[("c.jpg", b"ccc"), ("d.jpg", b"dddd"), ("e.jpg", b"eeeee")],
    )
    .await;
    rename(
        fixture,
        &[("c.jpg", "d.jpg"), ("d.jpg", "e.jpg"), ("e.jpg", "c.jpg")],
    )
    .await;
    // The cycle's second exchange.
    fixture.fs().fail_on(LocalOperation::Relocating, 2);

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    assert_eq!(outcome.relocations.len(), 3, "{:?}", outcome.relocations);
    assert!(
        outcome
            .relocations
            .iter()
            .any(|one| matches!(one, Relocation::Relocated { .. })),
        "the first exchange is made: {:?}",
        outcome.relocations
    );
    let refused_exchange = refusal(&outcome, "c.jpg", "e.jpg");
    assert!(
        matches!(refused_exchange, RelocationRefused::Disk(_)),
        "{refused_exchange:?}"
    );
    let rest = refusal(&outcome, "e.jpg", "c.jpg");
    assert!(
        matches!(rest, RelocationRefused::WaitingFileInTheWay),
        "{rest:?}"
    );

    let next = sync(fixture, 3).await;
    uploaded_nothing(&next);
    for (relative, bytes) in [
        ("d.jpg", &b"ccc"[..]),
        ("e.jpg", b"dddd"),
        ("c.jpg", b"eeeee"),
    ] {
        assert_eq!(
            content(fixture, relative).as_deref(),
            Some(bytes),
            "{relative}"
        );
    }
    assert!(sync(fixture, 4).await.relocations.is_empty());
}
