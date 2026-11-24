use coffret_model::ContainerKind;

use crate::device_state::{DeviceTime, LocalEntryState};
use crate::index_conformance::fixtures::{
    addition, container_id, entry, observation, observation_of, path, record,
};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_conformance::renames::{rename, renaming};

/// A file this device has, at a path the Library no longer holds, is reported.
///
/// Another device's commit can remove the Container an Entry lived in, and the
/// local file stays on this disk. Its row outlives the Entry, which is what
/// lets the device say it is there rather than leave it unnoticed — and it is
/// not swept away by the removal, because device state is not what a commit
/// changes (spec: CK-7, EP-10).
pub async fn a_file_left_behind_by_the_library_is_reported(fixture: &IndexUnderTest) {
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
        .expect("replaying a record must succeed");
    index
        .mark_present(observation_of("albums/a.jpg", 100, entry(1, 0)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_present(observation_of("albums/b.jpg", 101, entry(1, 1)))
        .await
        .expect("recording a materialized file must succeed");

    // Elsewhere, the Pack is replaced by one holding only `a.jpg`.
    index
        .apply(record(
            1,
            vec![addition(2, ContainerKind::Pack, &["albums/a.jpg"])],
            vec![container_id(1)],
        ))
        .await
        .expect("replaying a replacement must succeed");

    let left_behind = index
        .present_without_entry()
        .await
        .expect("reading what is left behind must succeed");
    assert_eq!(
        left_behind.len(),
        1,
        "only the removed path is left behind, got {left_behind:?}"
    );
    assert_eq!(left_behind[0].observation.path.as_str(), "albums/b.jpg");
    assert_eq!(left_behind[0].state, LocalEntryState::Present);

    let present = index
        .present_under(None)
        .await
        .expect("reading what this device has must succeed");
    assert_eq!(
        present.len(),
        2,
        "the row survives the removal, got {present:?}"
    );
    // The replacement at `a.jpg` takes the place of the Entry its file is a
    // copy of, so the record is carried over to it (spec: EP-6, EP-10).
    assert_eq!(present[0].observation.path.as_str(), "albums/a.jpg");
    assert_eq!(present[0].observation.entry, entry(2, 0));
    assert_eq!(present[1].observation.entry, entry(1, 1));
}

/// Only a file this device put in place can be reported as gone.
///
/// An Entry the device never materialized is outside its scope rather than
/// missing, mapped or not, so marking such a path absent records nothing: doing
/// otherwise would let a device propose the removal of a file it never had
/// (spec: EP-10).
pub async fn only_a_file_this_device_had_can_go_absent(fixture: &IndexUnderTest) {
    let index = fixture.index();
    let never_held = path("books/page-001.png");

    index
        .mark_absent(&never_held, DeviceTime::from_unix_seconds(1_700_000_900))
        .await
        .expect("marking an unheld path absent must succeed");
    assert!(
        index
            .local_entry_at(&never_held)
            .await
            .expect("reading a local row must succeed")
            .is_none(),
        "a path this device never materialized gets no row"
    );

    let held = path("albums/a.jpg");
    index
        .mark_present(observation("albums/a.jpg", 100))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_absent(&held, DeviceTime::from_unix_seconds(1_700_000_900))
        .await
        .expect("marking a held path absent must succeed");

    let row = index
        .local_entry_at(&held)
        .await
        .expect("reading a local row must succeed")
        .expect("a file this device had keeps its row");
    assert_eq!(row.state, LocalEntryState::Absent);
    assert_eq!(
        row.observation.size, 100,
        "the last look at the file is what the device knows of it"
    );
    assert_eq!(
        row.observation.at,
        DeviceTime::from_unix_seconds(1_700_000_900),
        "the time of looking moves to when it was found gone"
    );

    assert!(
        index
            .present_under(None)
            .await
            .expect("reading what this device has must succeed")
            .is_empty(),
        "a file that is gone is not one this device has"
    );
}

/// The content hash recorded beside a materialization comes back as it went
/// in, a row recorded with none comes back with none, and an absence keeps the
/// hash the row had (spec: EP-15).
///
/// The hash is the one fact about an Entry that outlives it here: a catch-up
/// that removes the Entry removes its hash from the catalog, and a scan still
/// has to tell a file left as the Library had it from one edited since.
pub async fn a_materialization_keeps_the_hash_it_was_recorded_with(fixture: &IndexUnderTest) {
    let index = fixture.index();
    let recorded = observation("albums/a.jpg", 100);
    index
        .mark_present(recorded.clone())
        .await
        .expect("recording a materialized file must succeed");

    let row = index
        .local_entry_at(&path("albums/a.jpg"))
        .await
        .expect("reading a local row must succeed")
        .expect("the row was recorded");
    assert_eq!(row.observation.hash, recorded.hash);

    index
        .mark_absent(
            &path("albums/a.jpg"),
            DeviceTime::from_unix_seconds(1_700_000_900),
        )
        .await
        .expect("marking a held path absent must succeed");
    let absent = index
        .local_entry_at(&path("albums/a.jpg"))
        .await
        .expect("reading a local row must succeed")
        .expect("a file this device had keeps its row");
    assert_eq!(
        absent.observation.hash, recorded.hash,
        "absence keeps the hash"
    );

    let unhashed = crate::device_state::LocalObservation {
        hash: None,
        ..observation("albums/b.jpg", 101)
    };
    index
        .mark_present(unhashed)
        .await
        .expect("recording a materialized file must succeed");
    let row = index
        .local_entry_at(&path("albums/b.jpg"))
        .await
        .expect("reading a local row must succeed")
        .expect("the row was recorded");
    assert_eq!(
        row.observation.hash, None,
        "a row with no hash reads back with none"
    );
}

/// Forgetting a row puts the path outside this device's scope again, and
/// forgetting one that is not there succeeds (spec: EP-15).
pub async fn a_forgotten_row_is_gone_and_forgetting_twice_succeeds(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .mark_present(observation("albums/a.jpg", 100))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_present(observation("albums/b.jpg", 101))
        .await
        .expect("recording a materialized file must succeed");

    for _ in 0..2 {
        index
            .forget_local_entry(&path("albums/a.jpg"))
            .await
            .expect("forgetting a row must succeed, there or not");
    }

    assert!(
        index
            .local_entry_at(&path("albums/a.jpg"))
            .await
            .expect("reading a local row must succeed")
            .is_none(),
        "the forgotten path has no row"
    );
    let present = index
        .present_under(None)
        .await
        .expect("reading what this device has must succeed");
    assert_eq!(
        present.len(),
        1,
        "the other row is untouched, got {present:?}"
    );
    assert_eq!(present[0].observation.path.as_str(), "albums/b.jpg");
}

/// The Entries under a prefix this device does not have are the subtree's
/// Entries less the ones it has on disk, read at once (spec: EP-10).
///
/// A file this device marked absent again, an Entry it never had, and an Entry
/// whose path holds a file waiting to follow a rename elsewhere are all among
/// them; a file it has is not, and nor is anything outside the prefix.
pub async fn the_entries_absent_under_a_prefix_are_the_ones_not_on_disk(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &[
                    "albums/a.jpg",
                    "albums/b.jpg",
                    "albums/c.jpg",
                    "books/d.png",
                ],
            )],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");
    index
        .mark_present(observation_of("albums/a.jpg", 100, entry(1, 0)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_present(observation_of("albums/b.jpg", 101, entry(1, 1)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_absent(
            &path("albums/b.jpg"),
            DeviceTime::from_unix_seconds(1_700_000_900),
        )
        .await
        .expect("recording a file gone must succeed");

    let absent = |prefix: Option<&'static str>| {
        let prefix = prefix.map(path);
        async move {
            index
                .entries_absent_under(prefix.as_ref())
                .await
                .expect("reading the Entries not on disk must succeed")
                .into_iter()
                .map(|location| location.path().as_str().to_owned())
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(
        absent(Some("albums")).await,
        ["albums/b.jpg", "albums/c.jpg"],
        "the one marked absent and the one never had, in path order",
    );
    assert_eq!(
        absent(None).await,
        ["albums/b.jpg", "albums/c.jpg", "books/d.png"],
        "the root covers every subtree",
    );

    // Swapping the two names leaves the file at albums/a.jpg waiting to follow
    // its Entry to albums/b.jpg: the Entry now at albums/a.jpg is not had there.
    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(1, 1, "albums/a.jpg")],
        ))
        .await
        .expect("applying the renames must succeed");
    assert_eq!(
        absent(Some("albums")).await,
        ["albums/a.jpg", "albums/b.jpg", "albums/c.jpg"],
        "a file waiting to follow its Entry elsewhere is not the Entry at its own path",
    );
}

/// A folder is whole on this device exactly where no current Entry standing
/// directly in it is remote, by the same materialization record a folder's
/// listing reads (spec: EP-10).
///
/// What a page opened from disk asks of its folder, and the answer has to be
/// [`folder_contents`](crate::Index::folder_contents)' own: an Entry never
/// had, one marked absent, and one whose file waits to follow its Entry
/// elsewhere are each not here, while nothing below the folder's child folders
/// counts either way — a folder of books is whole where its own files are,
/// whatever is missing from the books inside it.
pub async fn a_folder_is_whole_where_nothing_directly_in_it_is_remote(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &[
                    "albums/a.jpg",
                    "albums/b.jpg",
                    "albums/deep/c.jpg",
                    "books/d.png",
                ],
            )],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");

    let remote = |folder: Option<&'static str>| {
        let folder = folder.map(path);
        async move {
            let answer = index
                .any_remote_in(folder.as_ref())
                .await
                .expect("asking whether a folder is whole must succeed");
            let listed = index
                .folder_contents(folder.as_ref())
                .await
                .expect("reading the folder's contents must succeed")
                .files
                .iter()
                .any(|file| !file.present);
            assert_eq!(answer, listed, "the answer is the listing's own");
            answer
        }
    };
    assert!(remote(Some("albums")).await, "nothing is here yet");
    assert!(
        !remote(None).await,
        "the root holds no Entry directly, so none of its own is remote",
    );

    index
        .mark_present(observation_of("albums/a.jpg", 100, entry(1, 0)))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_present(observation_of("albums/b.jpg", 101, entry(1, 1)))
        .await
        .expect("recording a materialized file must succeed");
    assert!(
        !remote(Some("albums")).await,
        "every file directly in it is here, whatever its child folder lacks",
    );
    assert!(remote(Some("albums/deep")).await);
    assert!(remote(Some("books")).await);

    index
        .mark_absent(
            &path("albums/b.jpg"),
            DeviceTime::from_unix_seconds(1_700_000_900),
        )
        .await
        .expect("recording a file gone must succeed");
    assert!(
        remote(Some("albums")).await,
        "a file marked absent is not here"
    );

    index
        .mark_present(observation_of("albums/b.jpg", 101, entry(1, 1)))
        .await
        .expect("recording a materialized file must succeed");
    assert!(!remote(Some("albums")).await);
    // Swapping the two names leaves each file waiting to follow its Entry to
    // the other's path, so neither Entry is had at its own path.
    index
        .apply(renaming(
            record(1, vec![], vec![]),
            vec![rename(1, 0, "albums/b.jpg"), rename(1, 1, "albums/a.jpg")],
        ))
        .await
        .expect("applying the renames must succeed");
    assert!(
        remote(Some("albums")).await,
        "a file waiting to follow its Entry elsewhere is not the Entry at its own path",
    );
}

/// The present rows outside some top-level folders are what this device has on
/// disk, less every path whose top-level component is one of them: the
/// remainder a Library-root mapping represents once other mappings stand for
/// those folders (spec: EP-9, EP-10).
///
/// A sibling whose name merely begins with an excluded folder's is not inside
/// it, a file marked absent is not had, and leaving nothing out is every
/// materialization record that says present. A file standing at the Library
/// root under the very name of an excluded folder is left out with that folder,
/// as [`present_under`](crate::index::Index::present_under) takes it in for the
/// folder.
pub async fn the_present_rows_outside_some_folders_are_the_remainder(fixture: &IndexUnderTest) {
    let index = fixture.index();
    for (text, size) in [
        ("cover.jpg", 100),
        ("albums/a.jpg", 101),
        ("albums/deep/b.jpg", 102),
        ("albums-annex/c.jpg", 103),
        ("books/d.png", 104),
        ("misc/e.jpg", 105),
        ("misc/f.jpg", 106),
        ("notes", 107),
    ] {
        index
            .mark_present(observation(text, size))
            .await
            .expect("recording a materialized file must succeed");
    }
    index
        .mark_absent(
            &path("misc/f.jpg"),
            DeviceTime::from_unix_seconds(1_700_000_900),
        )
        .await
        .expect("recording a file gone must succeed");

    let outside = |excluded: &'static [&'static str]| async move {
        index
            .present_outside(&excluded.iter().copied().collect())
            .await
            .expect("reading what this device has outside some folders must succeed")
            .into_iter()
            .map(|local| local.observation.path.as_str().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        outside(&["albums", "books"]).await,
        ["albums-annex/c.jpg", "cover.jpg", "misc/e.jpg", "notes"],
        "the remainder, in path order",
    );
    assert_eq!(
        outside(&["albums", "books", "notes"]).await,
        ["albums-annex/c.jpg", "cover.jpg", "misc/e.jpg"],
        "the file at an excluded folder's own path is that folder's, as present_under takes it",
    );
    let under_notes: Vec<String> = index
        .present_under(Some(&path("notes")))
        .await
        .expect("reading what this device has under a prefix must succeed")
        .into_iter()
        .map(|local| local.observation.path.as_str().to_owned())
        .collect();
    assert_eq!(
        under_notes,
        ["notes"],
        "the boundary present_outside leaves out"
    );
    let every: Vec<String> = index
        .present_under(None)
        .await
        .expect("reading what this device has must succeed")
        .into_iter()
        .map(|local| local.observation.path.as_str().to_owned())
        .collect();
    assert_eq!(
        outside(&[]).await,
        every,
        "leaving nothing out is every row"
    );
}

/// The present rows in a folder are the ones standing directly in it — the
/// Library root's are the files with no folder at all — and none in the
/// folders below it or beside it (spec: EP-10).
///
/// A folder whose name is in multi-byte characters is bounded the same way.
pub async fn the_present_rows_in_a_folder_are_the_ones_directly_in_it(fixture: &IndexUnderTest) {
    let index = fixture.index();
    for (text, size) in [
        ("cover.jpg", 100),
        ("albums/a.jpg", 101),
        ("albums/deep/b.jpg", 102),
        ("albums-annex/c.jpg", 103),
        ("albums/e.jpg", 104),
        ("写真/f.jpg", 105),
        ("写真/深/g.jpg", 106),
    ] {
        index
            .mark_present(observation(text, size))
            .await
            .expect("recording a materialized file must succeed");
    }
    index
        .mark_absent(
            &path("albums/e.jpg"),
            DeviceTime::from_unix_seconds(1_700_000_900),
        )
        .await
        .expect("recording a file gone must succeed");

    let within = |folder: Option<&'static str>| {
        let folder = folder.map(path);
        async move {
            index
                .present_in(folder.as_ref())
                .await
                .expect("reading what this device has in one folder must succeed")
                .into_iter()
                .map(|local| local.observation.path.as_str().to_owned())
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(within(None).await, ["cover.jpg"], "the root's own files");
    assert_eq!(
        within(Some("albums")).await,
        ["albums/a.jpg"],
        "neither the folder below nor the sibling, nor the file marked absent",
    );
    assert_eq!(within(Some("albums/deep")).await, ["albums/deep/b.jpg"]);
    assert_eq!(within(Some("写真")).await, ["写真/f.jpg"]);
    assert!(within(Some("books")).await.is_empty());
}
