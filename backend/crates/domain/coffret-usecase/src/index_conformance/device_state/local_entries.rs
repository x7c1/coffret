use coffret_model::ContainerKind;

use crate::device_state::{DeviceTime, LocalEntryState};
use crate::index_conformance::fixtures::{addition, container_id, observation, path, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

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
        .mark_present(observation("albums/a.jpg", 100))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_present(observation("albums/b.jpg", 101))
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
