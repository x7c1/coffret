use coffret_model::{ContainerKind, Mtime};

use crate::device_state::Mapping;
use crate::entry_paths::entry_path;
use crate::sync::{sync_folders, Surfaced};
use crate::sync_conformance::fixtures::{keys, map, plant, request, write, OLDER};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A local file whose addition would make one Entry Path both a file and a
/// folder is left out of the run and reported, naming that path, and the other
/// files of the same run are still added (spec: EP-4, EP-6).
///
/// Both directions are asked against Entries another device added, which this
/// device never had: a file under a path an Entry stands at, and a file at a
/// path Entries stand under. A commit carrying either would be refused whole,
/// holding back every other file of the run, so the run leaves them out before
/// anything is packed or uploaded — and reports them again the next time,
/// until one of the two is renamed.
pub async fn a_file_that_would_make_a_path_both_a_file_and_a_folder_is_left_out(
    fixture: &SyncUnderTest,
) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    for path in ["docs/notes", "albums/a.jpg"] {
        plant(
            store,
            index,
            &keys,
            ContainerKind::OneFile,
            path,
            b"another device's file",
            Mtime::from_unix_seconds(OLDER),
            false,
        )
        .await;
    }
    map(fixture, None).await;
    write(
        fixture.fs(),
        fixture.folder(),
        "docs/notes/x.pdf",
        b"under a file",
    );
    write(fixture.fs(), fixture.folder(), "albums", b"over a folder");
    write(
        fixture.fs(),
        fixture.folder(),
        "docs/other.pdf",
        b"beside them",
    );
    write(fixture.fs(), fixture.folder(), "top.jpg", b"at the top");

    for run in [1, 2] {
        let outcome = sync_folders(request(store, index, &keys, fixture.fs(), run))
            .await
            .expect("a sync leaving files out must succeed");

        assert_eq!(
            left_out(&outcome.surfaced),
            [
                ("albums", "albums", false),
                ("docs/notes/x.pdf", "docs/notes", false)
            ],
            "run {run} reports each file left out and the path it would make both, \
             which no mapping of this device is mapped at",
        );
        assert_eq!(
            outcome.surfaced.len(),
            2,
            "and nothing else: {:?}",
            outcome.surfaced
        );
        if run == 1 {
            assert_eq!(
                outcome.added.len(),
                2,
                "the other files are added: {:?}",
                outcome.added
            );
            assert!(outcome.commit.is_some(), "and committed");
        } else {
            assert!(outcome.added.is_empty(), "nothing else is new");
            assert!(outcome.commit.is_none());
        }
    }

    for added in ["docs/other.pdf", "top.jpg"] {
        assert!(
            index
                .entry_at(&entry_path(added))
                .await
                .expect("asking the Index for a path must succeed")
                .is_some(),
            "{added} is in the Library",
        );
    }
    for left in ["docs/notes/x.pdf", "albums"] {
        assert!(
            index
                .entry_at(&entry_path(left))
                .await
                .expect("asking the Index for a path must succeed")
                .is_none(),
            "{left} is not",
        );
    }
}

/// Where the path that would be both a file and a folder is the name a
/// top-level mapping of this device is mapped at, the finding says so,
/// whichever of the two is left out (spec: EP-6, EP-9).
///
/// The folder of the two is then that mapping's, whose place in the Library is
/// the name it was mapped at and not its local folder's name, so renaming the
/// folder on disk would change nothing: whoever reads the finding out needs to
/// know which case it is to say what helps.
///
/// At `notes` the mapped folder's file is in the Library and a file at its name
/// appears in the root-mapped folder; at `books` it is the other way round, a
/// folder mapped at the name of a file already in the Library.
pub async fn a_file_left_out_beside_a_mapped_folder_says_the_folder_is_mapped(
    fixture: &SyncUnderTest,
) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    let remainder = fixture.folder().join("everything");
    let noted = fixture.folder().join("noted");
    let booked = fixture.folder().join("booked");
    write(fixture.fs(), &noted, "x.pdf", b"in the mapped folder");
    write(
        fixture.fs(),
        &remainder,
        "books",
        b"a file named as a folder will be",
    );
    for (prefix, root) in [(None, &remainder), (Some("notes"), &noted)] {
        index
            .set_mapping(Mapping::new(prefix.map(entry_path), root.clone()))
            .await
            .expect("recording a mapping must succeed");
    }
    let first = sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .expect("a first sync must succeed");
    assert_eq!(first.added.len(), 2, "{first:?}");

    write(
        fixture.fs(),
        &remainder,
        "notes",
        b"a file named as the folder",
    );
    write(
        fixture.fs(),
        &booked,
        "y.pdf",
        b"in the folder mapped later",
    );
    index
        .set_mapping(Mapping::new(Some(entry_path("books")), booked))
        .await
        .expect("recording a mapping must succeed");
    let second = sync_folders(request(store, index, &keys, fixture.fs(), 2))
        .await
        .expect("a sync leaving files out must succeed");

    assert_eq!(
        left_out(&second.surfaced),
        [("books/y.pdf", "books", true), ("notes", "notes", true)],
        "each file left out is reported beside the folder mapped at the path it would \
         make both",
    );
    assert!(
        second.commit.is_none(),
        "and nothing else is new: {second:?}"
    );
}

/// Each file a run left out for making a path both a file and a folder, the
/// path it would make both, and whether that path is one a mapping is mapped at.
fn left_out(surfaced: &[Surfaced]) -> Vec<(&str, &str, bool)> {
    surfaced
        .iter()
        .filter_map(|surfaced| match surfaced {
            Surfaced::FileAndFolder { path, both, mapped } => {
                Some((path.as_str(), both.as_str(), *mapped))
            }
            _ => None,
        })
        .collect()
}
