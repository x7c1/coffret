use std::ffi::OsStr;

use coffret_model::Mtime;

use crate::folder_entry::FolderEntry;
use crate::folder_entry_kind::FolderEntryKind;
use crate::local_operation::LocalOperation;
use crate::mapped_roots_conformance::mapped_roots_under_test::MappedRootsUnderTest;

/// A folder that is not there lists to nothing, and not to a failure.
///
/// The mirror of the missing root, and the answer the walk needs for a
/// subdirectory that went away between being listed and being descended into: it
/// holds no more files, which is no reason to fail a run over the folders that
/// are there. The case makes the folder and takes it away again, because that
/// disappearance — rather than a name nobody ever used — is the shape the walk
/// meets.
pub async fn a_missing_folder_lists_to_nothing(fixture: &MappedRootsUnderTest) {
    let gone = fixture.dir().join("photographs").join("2026");
    fixture.arrange().create_dir(&gone);
    fixture.arrange().remove_dir_all(&gone);

    let listing = fixture
        .roots()
        .list_folder(&gone)
        .await
        .expect("a folder that is not there is a verdict and not a failure");

    assert!(listing.is_none(), "nothing stands at the path any more");
}

/// Something that is at the path and is not a folder is refused, and never
/// listed as nothing.
///
/// The other half of the case above, and the reason `None` may stand for absence
/// alone: the walk reads it as the missing-root verdict, so a capability that
/// answered a mapped root somebody pointed at a file with nothing would report
/// an unplugged disk and quietly stop backing that mapping up (spec: EP-12). It
/// refuses what a real directory read refuses, and names the listing it was
/// refused for.
pub async fn listing_something_that_is_not_a_folder_is_refused(fixture: &MappedRootsUnderTest) {
    let path = fixture.dir().join("photographs").join("spring.jpg");
    fixture.arrange().write_file(&path, b"a photo", stamped());

    let refused = fixture
        .roots()
        .list_folder(&path)
        .await
        .expect_err("what stands at the path is a file and not a folder to list");

    assert!(
        matches!(refused.operation, LocalOperation::Listing),
        "the call that was refused was the listing, got {refused:?}",
    );
    assert_eq!(refused.path, path, "and it names the path it was about");
}

/// A listing says how long a file is and when it was last modified, and calls a
/// folder a folder.
///
/// The stat is the listing's and not a second call the walk makes, because the
/// two would otherwise be two looks at a folder that may have moved in between —
/// and because the times are what an Entry carries (spec: FM-9). The folder is
/// there so the two kinds are told apart by the same call.
pub async fn a_listing_reports_a_files_size_and_mtime_and_a_folder_as_a_folder(
    fixture: &MappedRootsUnderTest,
) {
    let root = fixture.dir().join("photographs");
    let content = b"a photo, as far as this case is concerned";
    fixture
        .arrange()
        .write_file(&root.join("spring.jpg"), content, stamped());
    fixture.arrange().create_dir(&root.join("2026"));

    let listing = listed(fixture, &root).await;
    assert_eq!(
        listing.len(),
        2,
        "the file and the folder, and nothing else"
    );

    let FolderEntryKind::File { size, mtime, .. } = named(&listing, "spring.jpg").kind else {
        panic!("a regular file is a file");
    };
    assert_eq!(size, content.len() as u64, "the file's own length");
    assert_eq!(mtime, stamped(), "and the time it was stamped with");

    assert!(
        matches!(named(&listing, "2026").kind, FolderEntryKind::Folder),
        "a directory is what the walk descends into",
    );
}

/// Something that is neither a file nor a folder lists as neither, and never as
/// what it points at.
///
/// A symbolic link is what this is about (spec: EP-8). Reporting it as the file
/// at the other end would give it an Entry Path of its own, and back up bytes
/// that live somewhere the mappings never pointed — and a dangling one would
/// simply fail the stat. So the listing states the name itself and answers with
/// the one kind that means "leave this alone".
pub async fn something_that_is_neither_a_file_nor_a_folder_lists_as_other(
    fixture: &MappedRootsUnderTest,
) {
    let root = fixture.dir().join("photographs");
    fixture
        .arrange()
        .write_file(&root.join("spring.jpg"), b"a photo", stamped());
    fixture.arrange().plant_other(&root.join("elsewhere"));

    let listing = listed(fixture, &root).await;

    assert!(
        matches!(named(&listing, "elsewhere").kind, FolderEntryKind::Other),
        "neither a file to carry into the Library nor a folder to descend into",
    );
    assert!(
        matches!(
            named(&listing, "spring.jpg").kind,
            FolderEntryKind::File { .. },
        ),
        "and the ordinary file beside it is unaffected",
    );
}

/// One listing of a folder the case expects to be there.
async fn listed(fixture: &MappedRootsUnderTest, dir: &std::path::Path) -> Vec<FolderEntry> {
    fixture
        .roots()
        .list_folder(dir)
        .await
        .unwrap_or_else(|error| panic!("listing a folder that is there must succeed: {error}"))
        .expect("the folder is there")
}

/// The one entry of a listing that goes by `name`, or a panic saying it is not
/// there.
///
/// A listing's order is the filesystem's own and means nothing, so every case
/// here reaches for a name rather than for a position.
fn named<'a>(listing: &'a [FolderEntry], name: &str) -> &'a FolderEntry {
    listing
        .iter()
        .find(|entry| entry.name == OsStr::new(name))
        .unwrap_or_else(|| panic!("the folder holds {name}"))
}

/// The moment the listing cases stamp their file with.
///
/// A fixed one rather than the clock's, because what a case asserts is that
/// *this* value came back rather than that some value did.
fn stamped() -> Mtime {
    Mtime::from_unix_seconds(1_600_000_000)
}
