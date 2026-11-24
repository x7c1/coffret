use std::path::{Path, PathBuf};

use crate::conformance_library::Library;
use crate::device_state::ReservationId;
use crate::entry_paths::entry_path;
use crate::freeze::{preview_freeze, FileAndFolder};
use crate::freeze_conformance::fixtures::{freeze, keys, map, write, TARGET};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;
use crate::local_scan::NONE_PLACED;

/// The file that would make `notes` both a file and a folder, and the folder
/// it would make it one with.
///
/// The Library-root mapping holds a file at `notes` beside `top.jpg`, and a
/// mapping of its own, at `notes`, holds a folder of one file. Adding both is
/// what no commit admits; in Entry Path order `notes` comes first, so it is
/// `notes/x.pdf` that would make `notes` both — and the folder of the two is
/// the one a mapping stands at.
fn stands_in_the_way() -> FileAndFolder {
    FileAndFolder {
        path: entry_path("notes/x.pdf"),
        both: entry_path("notes"),
        mapped: true,
    }
}

/// Lays out the two mapped folders [`stands_in_the_way`] describes, and
/// answers where the Library-root mapping's folder is.
async fn clashing(fixture: &FreezeUnderTest) -> PathBuf {
    let index = fixture.source();
    let remainder = fixture.source_folder().join("everything");
    let noted = fixture.source_folder().join("noted");
    write(
        fixture.fs(),
        &remainder,
        "notes",
        b"a file named as a folder",
    );
    write(fixture.fs(), &remainder, "top.jpg", b"at the top");
    write(fixture.fs(), &noted, "x.pdf", b"in the mapped folder");
    map(index, None, &remainder).await;
    map(index, Some("notes"), &noted).await;
    remainder
}

/// A drop's reservation of the mapped folder's one file, received
/// (spec: PK-17).
async fn reserved(fixture: &FreezeUnderTest) -> ReservationId {
    let index = fixture.source();
    let reservation = index
        .reserve(Some(&entry_path("notes")))
        .await
        .expect("recording a reservation must succeed");
    index
        .extend_reservation(reservation, &entry_path("notes/x.pdf"))
        .await
        .expect("extending a reservation must succeed");
    index
        .mark_received(reservation)
        .await
        .expect("marking a reservation received must succeed");
    reservation
}

/// Whether the source device's catalog holds an Entry at `path`.
async fn held(fixture: &FreezeUnderTest, path: &str) -> bool {
    fixture
        .source()
        .entry_at(&entry_path(path))
        .await
        .expect("asking the catalog for a path must succeed")
        .is_some()
}

/// EP-6: a freeze any file of which would make one Entry Path both a file and
/// a folder packs nothing and commits nothing, and names that file with the
/// path.
///
/// The two files that could be packed are not packed either, and no Pack and
/// no Journal record reaches Storage.
///
/// The preview asks the same question of the same selection first, and names
/// the same file, so a person is told before asking for a run that it would
/// be refused, and why.
pub async fn a_freeze_is_refused_whole_where_a_file_would_make_a_path_both_a_file_and_a_folder(
    fixture: &FreezeUnderTest,
) {
    let store = fixture.store();
    let keys = keys();
    clashing(fixture).await;
    let heads = Library::read(store).await.heads();

    let preview = preview_freeze(fixture.source(), fixture.fs(), &NONE_PLACED, None, None)
        .await
        .expect("the preview must succeed");
    assert_eq!(
        preview.refused,
        vec![stands_in_the_way()],
        "the preview names the file that stands in the way, as the run will",
    );

    let outcome = freeze(fixture, &keys, TARGET, 1).await;

    assert_eq!(
        outcome.refused,
        vec![stands_in_the_way()],
        "the run names the file, and the path it would make both, which the folder of the \
         two is mapped at",
    );
    assert_eq!(
        outcome.refused, preview.refused,
        "the run refuses exactly what the preview said it would",
    );
    assert!(outcome.packs.is_empty(), "nothing is packed");
    assert!(outcome.absorbed.is_empty());
    assert!(outcome.commit.is_none(), "nothing is committed");
    assert!(
        outcome.surfaced.is_empty(),
        "the refusal is not a file passed over: {:?}",
        outcome.surfaced,
    );
    assert_eq!(
        Library::read(store).await.heads(),
        heads,
        "no Journal record reached Storage",
    );
    assert!(
        fixture.fs().files_under(fixture.spool_dir()).is_empty(),
        "no Pack was spooled",
    );
    for path in ["notes", "top.jpg", "notes/x.pdf"] {
        assert!(!held(fixture, path).await, "{path} is not in the Library");
    }
}

/// EP-6, PK-17: a drop's reservation covering the file that refuses a freeze
/// is kept as it was through the refusal, and the freeze packs everything once
/// the names are changed.
///
/// The refused run decides no reservation — as any run that stops before
/// deciding one — so the book the drop asked for is still the one a later
/// freeze builds, rather than one released with nothing packed.
pub async fn a_reservation_is_kept_through_a_refused_freeze_and_packed_once_the_clash_is_gone(
    fixture: &FreezeUnderTest,
) {
    let index = fixture.source();
    let keys = keys();
    let remainder = clashing(fixture).await;
    let reservation = reserved(fixture).await;
    let before = index
        .reservations()
        .await
        .expect("reading the reservations must succeed");

    let refused = freeze(fixture, &keys, TARGET, 1).await;

    assert_eq!(refused.refused, vec![stands_in_the_way()]);
    assert_eq!(refused.released, 0, "no reservation is decided");
    assert_eq!(refused.lapsed, Vec::new(), "and none lapses");
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        before,
        "the reservation is kept as it was",
    );

    // The file at the name the mapping stands at is renamed, which is what
    // the refusal said helps: renaming the mapped folder on disk would not.
    rename(fixture, &remainder, "notes", "notes.txt");

    let packed = freeze(fixture, &keys, TARGET, 2).await;

    assert_eq!(packed.refused, Vec::new(), "nothing stands in the way");
    assert_eq!(packed.frozen_entries(), 3, "every file is packed");
    for path in ["notes.txt", "top.jpg", "notes/x.pdf"] {
        assert!(held(fixture, path).await, "{path} is in the Library");
    }
    assert!(!held(fixture, "notes").await);
    assert_eq!(
        packed.released, 1,
        "the reservation is decided by the commit"
    );
    assert!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed")
            .iter()
            .all(|kept| kept.id != reservation),
        "and released by it",
    );
}

/// Renames the file `from` in `folder` to `to`, as a person would on disk.
fn rename(fixture: &FreezeUnderTest, folder: &Path, from: &str, to: &str) {
    let fs = fixture.fs();
    let content = fs
        .content(&folder.join(from))
        .expect("the file to rename is there");
    fs.remove_file(&folder.join(from));
    fs.write_file(&folder.join(to), &content);
}
