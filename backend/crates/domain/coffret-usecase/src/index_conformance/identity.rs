//! Which Entry a materialization record is about (spec: EP-10).
//!
//! A record names the Entry its file is a copy of by `(Container ID, entry
//! number)`, and the catalog keeps it about that Entry: a replacement carries
//! it over, a rename has it wait to follow, and an Entry that comes to stand at
//! the file's path by any other way is never read as the one the file is a
//! copy of. The cases here are the shapes in which a record that knew only its
//! path would claim the wrong Entry, and the re-keying a relocation makes once
//! a file has followed its Entry.

use coffret_model::ContainerKind;

use crate::device_state::{DeviceTime, LocalEntry};
use crate::index::Index;
use crate::index_conformance::fixtures::{
    addition, container_id, entry, observation_of, path, record, snapshot,
};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_conformance::renames::{rename, renaming};

/// A file whose Entry left the Library is departed at its path, and a later
/// record that renames another Entry onto that path does not make the file a
/// copy of it: the row is forgotten rather than claim the newcomer present, so
/// no scan offers the file as its edit (spec: EP-10, EP-15, FM-15).
pub async fn a_departed_file_is_forgotten_where_a_rename_brings_another_entry(
    fixture: &IndexUnderTest,
) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![
                addition(1, ContainerKind::OneFile, &["albums/a.jpg"]),
                addition(3, ContainerKind::OneFile, &["inbox/c.jpg"]),
            ],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    index
        .mark_present(observation_of("albums/a.jpg", 100, entry(1, 0)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .apply(record(1, vec![], vec![container_id(1)]))
        .await
        .expect("applying the removal must succeed");
    assert_eq!(
        left_behind(index).await,
        ["albums/a.jpg"],
        "the file is departed once its Entry has left"
    );

    index
        .apply(renaming(
            record(2, vec![], vec![]),
            vec![rename(3, 0, "albums/a.jpg")],
        ))
        .await
        .expect("applying the rename must succeed");

    assert_eq!(row(index, "albums/a.jpg").await, None);
    assert!(
        present(index).await.is_empty(),
        "nothing stands for the renamed Entry at its new path"
    );
    assert!(left_behind(index).await.is_empty());
}

/// An addition at a departed file's path in a later record is another Entry
/// too, however it is named: only a replacement within one record — the
/// Container removed and the path's new Entry added by the same record —
/// carries a record over (spec: EP-6, EP-10).
pub async fn an_addition_after_a_removal_is_not_the_departed_files_entry(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![addition(1, ContainerKind::OneFile, &["albums/a.jpg"])],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    index
        .mark_present(observation_of("albums/a.jpg", 100, entry(1, 0)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .apply(record(1, vec![], vec![container_id(1)]))
        .await
        .expect("applying the removal must succeed");
    index
        .apply(record(
            2,
            vec![addition(2, ContainerKind::OneFile, &["albums/a.jpg"])],
            vec![],
        ))
        .await
        .expect("applying the later addition must succeed");

    assert_eq!(row(index, "albums/a.jpg").await, None);
}

/// Adopting a Snapshot past two Entries that swapped their names and the
/// removal of one of them claims neither for the wrong file: the file of the
/// removed Entry is forgotten under the one that took its path, and the other
/// file waits to follow its own Entry there. Nothing in the Snapshot says which
/// file is which by path; the record says it by name (spec: CK-9, EP-10,
/// FM-15).
pub async fn a_restore_past_a_swap_and_a_removal_claims_neither_entry(fixture: &IndexUnderTest) {
    let index = fixture.index();
    let other = fixture.other();
    let first = record(
        0,
        vec![
            addition(1, ContainerKind::OneFile, &["albums/a.jpg"]),
            addition(2, ContainerKind::OneFile, &["albums/b.jpg"]),
        ],
        vec![],
    );
    for later in [
        first.clone(),
        renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(2, 0, "albums/a.jpg")],
        ),
        record(2, vec![], vec![container_id(1)]),
    ] {
        other
            .apply(later)
            .await
            .expect("replaying a record must succeed");
    }
    index
        .apply(first)
        .await
        .expect("applying the first record must succeed");
    for (text, seed) in [("albums/a.jpg", 1), ("albums/b.jpg", 2)] {
        index
            .mark_present(observation_of(text, 100, entry(seed, 0)))
            .await
            .expect("recording a materialized file must succeed");
    }

    index
        .restore(
            other
                .snapshot()
                .await
                .expect("a replayed catalog has a state to checkpoint"),
        )
        .await
        .expect("adopting the later Snapshot must succeed");

    assert_eq!(
        row(index, "albums/a.jpg").await,
        None,
        "the file of the removed Entry is not a copy of the one now at its path"
    );
    let waiting = row(index, "albums/b.jpg")
        .await
        .expect("the other file keeps its row");
    assert_eq!(waiting.observation.entry, entry(2, 0));
    assert_eq!(waiting.relocate_to, Some(path("albums/a.jpg")));
    assert!(present(index).await.is_empty());
    assert!(left_behind(index).await.is_empty());
}

/// Adopting a Snapshot past an update and past a freeze carries each record
/// over to the Entry that took its Entry's place — an Entry in a Container the
/// replaced catalog did not hold, at the path its Entry stood at — so the file
/// is still this device's copy, as a replay of the records in between would
/// have left it (spec: CK-9, EP-6, EP-10).
pub async fn a_restore_carries_a_record_over_to_a_replacement(fixture: &IndexUnderTest) {
    let index = fixture.index();
    let first = record(
        0,
        vec![
            addition(1, ContainerKind::OneFile, &["albums/a.jpg"]),
            addition(2, ContainerKind::OneFile, &["albums/b.jpg"]),
            addition(3, ContainerKind::OneFile, &["albums/c.jpg"]),
        ],
        vec![],
    );
    index
        .apply(first)
        .await
        .expect("applying the first record must succeed");
    for (text, seed) in [
        ("albums/a.jpg", 1),
        ("albums/b.jpg", 2),
        ("albums/c.jpg", 3),
    ] {
        index
            .mark_present(observation_of(text, 100, entry(seed, 0)))
            .await
            .expect("recording a materialized file must succeed");
    }

    // `a.jpg` updated into Container 4, and `b.jpg` and `c.jpg` absorbed into
    // Pack 5, all after this device's catalog stopped.
    index
        .restore(snapshot(
            3,
            vec![
                addition(4, ContainerKind::OneFile, &["albums/a.jpg"]),
                addition(5, ContainerKind::Pack, &["albums/b.jpg", "albums/c.jpg"]),
            ],
            None,
        ))
        .await
        .expect("adopting the later Snapshot must succeed");

    let carried: Vec<_> = present(index)
        .await
        .into_iter()
        .map(|local| (local.observation.path, local.observation.entry))
        .collect();
    assert_eq!(
        carried,
        [
            (path("albums/a.jpg"), entry(4, 0)),
            (path("albums/b.jpg"), entry(5, 0)),
            (path("albums/c.jpg"), entry(5, 1)),
        ]
    );
}

/// A record names the Entry its file is a copy of, wherever the catalog holds
/// that Entry: one recorded for an Entry standing at another path waits to
/// follow it there, and both such rows and absent ones are what a run that
/// relocates reads (spec: EP-7, EP-10).
pub async fn a_record_of_an_entry_standing_elsewhere_waits_for_it(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &["albums/a.jpg", "albums/b.jpg"],
            )],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    index
        .mark_present(observation_of("inbox/a.jpg", 100, entry(1, 0)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_present(observation_of("inbox/b.jpg", 101, entry(1, 1)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_absent(&path("inbox/b.jpg"), DeviceTime::from_unix_seconds(1))
        .await
        .expect("recording a file as gone must succeed");

    let waiting: Vec<_> = index
        .awaiting_relocation()
        .await
        .expect("reading what waits to be relocated must succeed")
        .into_iter()
        .map(|local| {
            (
                local.observation.path.as_str().to_owned(),
                local.relocate_to.map(|to| to.as_str().to_owned()),
            )
        })
        .collect();
    assert_eq!(
        waiting,
        [
            ("inbox/a.jpg".to_owned(), Some("albums/a.jpg".to_owned())),
            ("inbox/b.jpg".to_owned(), Some("albums/b.jpg".to_owned())),
        ]
    );
    assert!(present(index).await.is_empty());
}

/// Re-keying a relocated file's record takes the row to the path its file now
/// stands at, keeping what the observation said, and the row then waits for
/// nothing where its Entry stands there. Two rows exchange their paths in one
/// call, a row already at a destination is replaced, and a relocation whose row is
/// gone is passed over (spec: EP-10).
pub async fn a_relocated_record_follows_its_file(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"],
            )],
            vec![],
        ))
        .await
        .expect("applying the first record must succeed");
    for (position, text) in ["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"]
        .into_iter()
        .enumerate()
    {
        index
            .mark_present(observation_of(
                text,
                100 + position as u64,
                entry(1, position),
            ))
            .await
            .expect("recording a materialized file must succeed");
    }
    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![
                rename(1, 0, "albums/b.jpg"),
                rename(1, 1, "albums/a.jpg"),
                rename(1, 2, "2024/c.jpg"),
            ],
        ))
        .await
        .expect("applying the renames must succeed");
    let before_c = row(index, "albums/c.jpg")
        .await
        .expect("the file keeps its row while it waits");

    index
        .relocate_local_entries(&[
            (path("albums/a.jpg"), path("albums/b.jpg")),
            (path("albums/b.jpg"), path("albums/a.jpg")),
        ])
        .await
        .expect("exchanging two rows must succeed");
    index
        .relocate_local_entries(&[
            (path("albums/c.jpg"), path("2024/c.jpg")),
            (path("albums/z.jpg"), path("2024/z.jpg")),
        ])
        .await
        .expect("re-keying a row must succeed");

    let rows: Vec<_> = present(index)
        .await
        .into_iter()
        .map(|local| (local.observation.path, local.observation.entry))
        .collect();
    assert_eq!(
        rows,
        [
            (path("2024/c.jpg"), entry(1, 2)),
            (path("albums/a.jpg"), entry(1, 1)),
            (path("albums/b.jpg"), entry(1, 0)),
        ],
        "every file is a copy of the Entry at its own path again"
    );
    let relocated = row(index, "2024/c.jpg")
        .await
        .expect("the relocated file's row stands at its new path");
    assert_eq!(relocated.observation.size, before_c.observation.size);
    assert_eq!(relocated.observation.mtime, before_c.observation.mtime);
    assert_eq!(relocated.observation.hash, before_c.observation.hash);
    assert_eq!(row(index, "albums/c.jpg").await, None);
    assert_eq!(row(index, "2024/z.jpg").await, None);
    assert!(index
        .awaiting_relocation()
        .await
        .expect("reading what waits to be relocated must succeed")
        .is_empty());

    // A row at the destination is replaced by the file relocated there.
    index
        .mark_present(observation_of("inbox/a.jpg", 300, entry(1, 1)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .relocate_local_entries(&[(path("inbox/a.jpg"), path("albums/a.jpg"))])
        .await
        .expect("re-keying onto a row must succeed");
    let replaced = row(index, "albums/a.jpg")
        .await
        .expect("the relocated row stands at the destination");
    assert_eq!(replaced.observation.size, 300);
    assert_eq!(row(index, "inbox/a.jpg").await, None);
}

/// The row at `text`, if this device keeps one.
async fn row(index: &dyn Index, text: &str) -> Option<LocalEntry> {
    index
        .local_entry_at(&path(text))
        .await
        .expect("reading a local file's row must succeed")
}

/// Every file this device has as a copy of the Entry at its own path.
async fn present(index: &dyn Index) -> Vec<LocalEntry> {
    index
        .present_under(None)
        .await
        .expect("reading what this device has must succeed")
}

/// The paths of the files the Library left behind.
async fn left_behind(index: &dyn Index) -> Vec<String> {
    index
        .present_without_entry()
        .await
        .expect("reading what the Library left behind must succeed")
        .into_iter()
        .map(|local| local.observation.path.as_str().to_owned())
        .collect()
}
