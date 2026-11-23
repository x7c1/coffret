//! What the folders of the Library come to, against what lies below them.
//!
//! An adapter may keep each folder's figures apart from the Entries they count,
//! so that a listing reads a folder's own rows rather than its subtree
//! (spec: LA-45). Figures kept apart are figures that can drift: a write that
//! changes an Entry or a present record and forgets to recount the folders
//! above it leaves a count the subtree no longer adds up to. So after every
//! kind of write the catalog takes, the case recounts each folder out of the
//! subtree reads and holds the port's answers to them.

use std::iter;

use coffret_model::{ContainerKind, EntryPath};

use crate::committed_batch::CommittedBatch;
use crate::device_state::DeviceTime;
use crate::folder_figures::{derive_folder_contents, derive_folders, derive_library_summary};
use crate::index::Index;
use crate::index_conformance::fixtures::{
    addition, container_id, entry, observation_of, path, record, snapshot,
};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_conformance::renames::{rename, renaming};

/// Holds every answer the port gives about folders to the recount of it out of
/// [`entries_under`](Index::entries_under),
/// [`present_under`](Index::present_under) and
/// [`containers_under`](Index::containers_under): the Library's totals, the
/// list of its folders, and the contents of the root and of every folder.
///
/// `after` names the write the catalog last took, for the failure to say.
pub async fn assert_folder_figures_recount(index: &dyn Index, after: &str) {
    assert_eq!(
        index
            .library_summary()
            .await
            .expect("reading the Library's totals must succeed"),
        derive_library_summary(index)
            .await
            .expect("recounting the Library must succeed"),
        "the Library's totals after {after}",
    );
    let folders = index
        .folders()
        .await
        .expect("reading the folders must succeed");
    assert_eq!(
        folders,
        derive_folders(index)
            .await
            .expect("recounting the folders must succeed"),
        "the Library's folders after {after}",
    );
    for folder in iter::once(None).chain(folders.iter().map(Some)) {
        assert_eq!(
            index
                .folder_contents(folder)
                .await
                .expect("reading a folder's contents must succeed"),
            derive_folder_contents(index, folder)
                .await
                .expect("recounting a folder must succeed"),
            "the contents of {} after {after}",
            folder.map_or("the root", EntryPath::as_str),
        );
    }
}

/// LA-45, EP-10: each folder's figures add up to the Entries below it and the
/// present records among them after every kind of write — a replay, this
/// device's own commit, one completed after another process replayed its
/// record, a file marked present, gone and forgotten, a rename across folders
/// and the relocation of its file, a removal that empties a folder, and a
/// restore.
pub async fn the_folder_figures_add_up_to_what_lies_below(fixture: &IndexUnderTest) {
    let index = fixture.index();
    assert_folder_figures_recount(index, "nothing").await;

    let paths = [
        "albums/2026/spring.jpg",
        "albums/2026/summer.jpg",
        "albums/cover.png",
        "books/novel/page-001.png",
        "books/novel/page-002.png",
        "books-annex/a.png",
        "notes.txt",
    ];
    index
        .apply(record(
            0,
            vec![addition(1, ContainerKind::Pack, &paths)],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    assert_folder_figures_recount(index, "a replay").await;

    for position in [0, 2, 3, 6] {
        index
            .mark_present(observation_of(paths[position], 100, entry(1, position)))
            .await
            .expect("recording a materialized file must succeed");
    }
    assert_folder_figures_recount(index, "files marked present").await;

    index
        .mark_absent(
            &path("albums/2026/spring.jpg"),
            DeviceTime::from_unix_seconds(1_700_000_600),
        )
        .await
        .expect("recording a file as gone must succeed");
    assert_folder_figures_recount(index, "a file gone").await;
    index
        .forget_local_entry(&path("albums/cover.png"))
        .await
        .expect("forgetting a row must succeed");
    assert_folder_figures_recount(index, "a row forgotten").await;
    index
        .mark_present(observation_of(paths[0], 100, entry(1, 0)))
        .await
        .expect("recording a materialized file must succeed");

    index
        .refresh(CommittedBatch {
            record: renaming(
                record(
                    1,
                    vec![addition(
                        2,
                        ContainerKind::OneFile,
                        &["albums/2025/winter.jpg"],
                    )],
                    vec![],
                ),
                vec![
                    rename(1, 0, "albums/2025/spring.jpg"),
                    rename(1, 3, "novel-page.png"),
                ],
            ),
            materialized: vec![observation_of("albums/2025/winter.jpg", 100, entry(2, 0))],
            released: Vec::new(),
        })
        .await
        .expect("refreshing after a commit must succeed");
    assert_folder_figures_recount(index, "a commit with renames across folders").await;

    index
        .relocate_local_entries(&[(
            path("albums/2026/spring.jpg"),
            path("albums/2025/spring.jpg"),
        )])
        .await
        .expect("re-keying a relocated file's row must succeed");
    assert_folder_figures_recount(index, "a relocation").await;

    // This device's own commit whose record another process replayed first:
    // the files it put on disk become present without the record being
    // applied again.
    let replayed = record(
        2,
        vec![addition(
            4,
            ContainerKind::OneFile,
            &["books/novel/page-003.png"],
        )],
        vec![],
    );
    index
        .apply(replayed.clone())
        .await
        .expect("replaying this device's own record must succeed");
    index
        .complete_applied(CommittedBatch {
            record: replayed,
            materialized: vec![observation_of("books/novel/page-003.png", 100, entry(4, 0))],
            released: Vec::new(),
        })
        .await
        .expect("completing a batch whose record is applied must succeed");
    assert_folder_figures_recount(index, "a commit completed after its replay").await;

    index
        .apply(record(3, vec![], vec![container_id(2)]))
        .await
        .expect("applying a removal must succeed");
    assert_folder_figures_recount(index, "a removal").await;

    index
        .restore(snapshot(
            5,
            vec![
                addition(1, ContainerKind::Pack, &paths),
                addition(3, ContainerKind::OneFile, &["books/novel/page-003.png"]),
            ],
            None,
        ))
        .await
        .expect("restoring a Snapshot must succeed");
    assert_folder_figures_recount(index, "a restore").await;

    index
        .apply(record(6, vec![], vec![container_id(1), container_id(3)]))
        .await
        .expect("applying a removal of everything must succeed");
    assert_folder_figures_recount(index, "a removal of every Container").await;
    assert!(
        index
            .folders()
            .await
            .expect("reading the folders must succeed")
            .is_empty(),
        "a Library holding nothing has no folder",
    );
}
