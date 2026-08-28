use coffret_model::ContainerKind;

use crate::device_state::{DeviceTime, LocalEntryState, PendingUpload, SpoolState};
use crate::index::Index;
use crate::index_conformance::fixtures::{
    addition, container_id, mapping, observation, path, pending, record, snapshot, snapshot_name,
    spooling, stamped,
};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// The identity a scan stamped one seeded mapping with (spec: EP-12).
const STAMPED: &str = "unix-dev:2049";

/// Puts one of each piece of device state into a catalog.
///
/// The mapping carries a recorded filesystem identity, so every case that
/// asserts device state came through untouched covers that column too.
pub(super) async fn seed_device_state(index: &dyn Index) {
    index
        .set_mapping(stamped(Some("albums"), "/photos", STAMPED))
        .await
        .expect("recording a mapping must succeed");
    index
        .mark_present(observation("albums/a.jpg", 100))
        .await
        .expect("recording a materialized file must succeed");
    index
        .record_pending_upload(pending(7, "batch-alpha"))
        .await
        .expect("recording a spool must succeed");
}

/// Asserts that seeded device state is exactly as it was left.
pub(super) async fn assert_device_state_intact(index: &dyn Index) {
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [stamped(Some("albums"), "/photos", STAMPED)]
    );
    let present = index
        .present_under(None)
        .await
        .expect("reading what this device has must succeed");
    assert_eq!(
        present.len(),
        1,
        "one file was materialized, got {present:?}"
    );
    assert_eq!(present[0].observation, observation("albums/a.jpg", 100));
    assert_eq!(
        index
            .pending_uploads()
            .await
            .expect("reading the spools must succeed"),
        [pending(7, "batch-alpha")]
    );
}

/// Adopting another device's Snapshot leaves this device's own state alone.
///
/// A Snapshot carries the Library and nothing of the device that wrote it — no
/// mappings, no local paths, no record of what anyone materialized, no spool
/// locations — so adopting one can neither bring another device's arrangement
/// in nor sweep this one's away (spec: CK-7, CK-9, EP-9, EP-10).
pub async fn a_restore_leaves_device_state_alone(fixture: &IndexUnderTest) {
    let index = fixture.index();
    seed_device_state(index).await;

    index
        .restore(snapshot(
            4,
            vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
            Some(snapshot_name(4)),
        ))
        .await
        .expect("restoring a Snapshot must succeed");

    assert_device_state_intact(index).await;
}

/// Replaying a record leaves this device's own state alone, for the same
/// reason a restore does (spec: CK-7, CP-11).
pub async fn a_replay_leaves_device_state_alone(fixture: &IndexUnderTest) {
    let index = fixture.index();
    seed_device_state(index).await;

    index
        .apply(record(
            0,
            vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");

    assert_device_state_intact(index).await;
}

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

/// Mappings are kept one per prefix, the Library root first (spec: EP-9).
pub async fn a_mapping_is_kept_once_per_prefix(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .set_mapping(mapping(Some("albums"), "/photos"))
        .await
        .expect("recording a mapping must succeed");
    index
        .set_mapping(mapping(None, "/data/library"))
        .await
        .expect("recording a root mapping must succeed");
    index
        .set_mapping(mapping(Some("albums"), "/photos-on-the-other-disk"))
        .await
        .expect("moving a mapping must succeed");

    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [
            mapping(None, "/data/library"),
            mapping(Some("albums"), "/photos-on-the-other-disk"),
        ]
    );
}

/// A mapping's recorded filesystem identity round-trips, and recording the
/// mapping afresh clears it (spec: EP-12).
///
/// The clearing is the half that matters, and it is not a convenience: it is the
/// gesture a device is left with when a folder it genuinely emptied stands on a
/// filesystem whose identity also moved. Such a root reports unavailable and
/// keeps doing so, because an empty root is never re-stamped — so recording the
/// mapping again with no identity is how the device says "this root is what I
/// meant", and the next scan stamps what is there and infers the deletions. A
/// catalog that kept the old identity through that call would leave the folder
/// stuck reporting unavailable forever.
pub async fn a_mapping_round_trips_its_root_identity(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .set_mapping(stamped(Some("albums"), "/photos", "unix-dev:2049"))
        .await
        .expect("recording a stamped mapping must succeed");
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [stamped(Some("albums"), "/photos", "unix-dev:2049")],
        "a mapping read back is the mapping that was written, its identity included",
    );

    // The disk came back renumbered, and a scan re-stamped what it saw.
    index
        .set_mapping(stamped(Some("albums"), "/photos", "unix-dev:2081"))
        .await
        .expect("re-stamping a mapping must succeed");
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [stamped(Some("albums"), "/photos", "unix-dev:2081")],
        "the identity moves with the rest of the row",
    );

    // The re-confirmation gesture: the same mapping, recorded with no identity.
    index
        .set_mapping(mapping(Some("albums"), "/photos"))
        .await
        .expect("recording a mapping afresh must succeed");
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [mapping(Some("albums"), "/photos")],
        "recording a mapping afresh clears the identity rather than keeping it",
    );
}

/// A Spooling row becomes Spooled when its spool file does, and only then
/// (spec: OC-2).
///
/// The row is written before the file it names exists, so what the catalog has
/// to carry is the difference between a spool file this device announced and one
/// it finished — the first being ciphertext worth nothing to anybody and the
/// second the only kind that is ever uploaded or committed.
///
/// Marking is a narrow flip and not an upsert: everything the announcing row
/// said about the Container is left alone, because none of it changed when the
/// file did. Marking an already-Spooled row changes nothing, so an interrupted
/// spool step is simply run again (spec: OC-6). And marking a Container the
/// catalog holds no row for changes nothing either, rather than failing or
/// inventing one — a row is what a spool step announced, and the operation says
/// that such a row's file is whole.
pub async fn a_spooling_row_becomes_spooled_when_its_file_completes(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .record_pending_upload(spooling(1, "batch-alpha"))
        .await
        .expect("recording a Spooling row must succeed");
    assert_eq!(
        index
            .pending_uploads()
            .await
            .expect("reading the spools must succeed"),
        [spooling(1, "batch-alpha")],
        "a row read back is the row that was written, its state included",
    );

    index
        .mark_spooled(container_id(1))
        .await
        .expect("marking the Container spooled must succeed");
    assert_eq!(
        index
            .pending_uploads()
            .await
            .expect("reading the spools must succeed"),
        [PendingUpload {
            state: SpoolState::Spooled,
            ..spooling(1, "batch-alpha")
        }],
        "the state moves and nothing else does",
    );

    index
        .mark_spooled(container_id(1))
        .await
        .expect("marking an already-Spooled row must succeed");
    index
        .mark_spooled(container_id(9))
        .await
        .expect("marking a Container with no row must succeed");
    assert_eq!(
        index
            .pending_uploads()
            .await
            .expect("reading the spools must succeed"),
        [PendingUpload {
            state: SpoolState::Spooled,
            ..spooling(1, "batch-alpha")
        }],
        "neither repeating the flip nor marking an unannounced spool changes the catalog",
    );
}

/// A spool is recorded until its batch settles, and clearing it twice is not an
/// error (spec: OC-2, OC-6).
///
/// The rows round-trip whole, their states included: what a later run reads is
/// what the run that spooled wrote down.
pub async fn a_spool_is_recorded_until_its_batch_settles(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .record_pending_upload(pending(1, "batch-alpha"))
        .await
        .expect("recording a spool must succeed");
    index
        .record_pending_upload(pending(2, "batch-alpha"))
        .await
        .expect("recording a second spool must succeed");
    assert_eq!(
        index
            .pending_uploads()
            .await
            .expect("reading the spools must succeed"),
        [pending(1, "batch-alpha"), pending(2, "batch-alpha")]
    );

    index
        .clear_pending_upload(container_id(1))
        .await
        .expect("clearing a spool must succeed");
    index
        .clear_pending_upload(container_id(1))
        .await
        .expect("clearing a spool already cleared must succeed");

    assert_eq!(
        index
            .pending_uploads()
            .await
            .expect("reading the spools must succeed"),
        [pending(2, "batch-alpha")]
    );
}
