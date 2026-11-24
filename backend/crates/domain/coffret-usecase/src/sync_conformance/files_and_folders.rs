use coffret_model::{ContainerKind, Mtime};

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

        let left_out: Vec<(&str, &str)> = outcome
            .surfaced
            .iter()
            .filter_map(|surfaced| match surfaced {
                Surfaced::FileAndFolder { path, both } => Some((path.as_str(), both.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            left_out,
            [("albums", "albums"), ("docs/notes/x.pdf", "docs/notes")],
            "run {run} reports each file left out and the path it would make both",
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
