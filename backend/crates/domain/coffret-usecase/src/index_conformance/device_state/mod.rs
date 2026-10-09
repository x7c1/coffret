use coffret_model::ContainerKind;

use crate::index::Index;
use crate::index_conformance::fixtures::{
    addition, observation, pending, record, snapshot, snapshot_name, stamped,
};
use crate::index_conformance::index_under_test::IndexUnderTest;

// The device-local half of the catalog, a module to each kind of row it keeps:
// what this device materialized, where it maps the Library to, the spools it
// has announced, and the parcels it holds. The cases that hold all four at once
// against a restore or a replay stay here, beside the seed they share with the
// refusal cases.
mod held_parcels;
pub use held_parcels::a_held_parcel_is_recorded_until_it_is_let_go;

mod local_entries;
pub use local_entries::{
    a_file_left_behind_by_the_library_is_reported,
    a_forgotten_row_is_gone_and_forgetting_twice_succeeds,
    a_materialization_keeps_the_hash_it_was_recorded_with,
    only_a_file_this_device_had_can_go_absent,
};

mod mappings;
pub use mappings::{a_mapping_is_kept_once_per_prefix, a_mapping_round_trips_its_root_identity};

mod pending_rows;
pub use pending_rows::{
    a_spool_is_recorded_until_its_row_is_cleared,
    a_spooling_row_becomes_spooled_when_its_file_completes,
    a_spooling_row_that_names_an_object_is_refused,
};

/// The identity a scan stamped one seeded mapping with (spec: EP-12).
///
/// Deliberately not any platform's own spelling of one. A [`RootIdentity`] is
/// opaque to a catalog — it is written down, read back, and compared, and never
/// parsed — so what these cases need of it is a string that comes out as it went
/// in, and using a real device's form would suggest the Index knew what it meant.
///
/// [`RootIdentity`]: crate::device_state::RootIdentity
const STAMPED: &str = "the-filesystem-a-scan-saw";

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
        .record_pending_row(pending(7, "batch-alpha"))
        .await
        .expect("recording a spool must succeed");
    index
        .hold_parcel(held_parcel())
        .await
        .expect("recording a held parcel must succeed");
}

/// The one held parcel the seed records (spec: PK-21).
fn held_parcel() -> crate::device_state::HeldParcel {
    crate::device_state::HeldParcel {
        container_id: crate::index_conformance::fixtures::container_id(7),
        index: 1,
        plaintext: 1024..2048,
        path: std::path::PathBuf::from("/state/parcels/7-1"),
    }
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
            .pending_rows()
            .await
            .expect("reading the spools must succeed"),
        [pending(7, "batch-alpha")]
    );
    assert_eq!(
        index
            .held_parcels()
            .await
            .expect("reading the held parcels must succeed"),
        [held_parcel()]
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
