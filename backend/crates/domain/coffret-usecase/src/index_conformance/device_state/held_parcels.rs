use std::path::PathBuf;

use crate::device_state::HeldParcel;
use crate::index_conformance::fixtures::container_id;
use crate::index_conformance::index_under_test::IndexUnderTest;

/// One held parcel of the Container numbered `container`.
fn parcel(container: u8, index: u64) -> HeldParcel {
    HeldParcel {
        container_id: container_id(container),
        index,
        plaintext: index * 1024..(index + 1) * 1024,
        path: PathBuf::from(format!("/state/parcels/{container}-{index}")),
    }
}

/// A held parcel is recorded until it is let go, once per parcel, and letting
/// one go twice succeeds (spec: PK-21, OC-8).
///
/// The rows are the only handle this device has on the parcel files it keeps,
/// so a catalog must hand back exactly what was recorded — the stretch of the
/// plaintext stream the parcel opens into says which Entries it covers, and the
/// path is where the file is — ordered by Container and then by parcel.
/// Recording one parcel again replaces its row rather than adding a second.
pub async fn a_held_parcel_is_recorded_until_it_is_let_go(fixture: &IndexUnderTest) {
    let index = fixture.index();
    for held in [parcel(2, 0), parcel(1, 3), parcel(1, 1)] {
        index
            .hold_parcel(held)
            .await
            .expect("recording a held parcel must succeed");
    }
    let moved = HeldParcel {
        path: PathBuf::from("/state/parcels/elsewhere"),
        ..parcel(1, 3)
    };
    index
        .hold_parcel(moved.clone())
        .await
        .expect("recording a held parcel again must succeed");
    assert_eq!(
        index
            .held_parcels()
            .await
            .expect("reading the held parcels must succeed"),
        [parcel(1, 1), moved, parcel(2, 0)],
        "one row per parcel, in Container and parcel order",
    );

    index
        .let_go_parcel(container_id(1), 3)
        .await
        .expect("letting go of a held parcel must succeed");
    index
        .let_go_parcel(container_id(1), 3)
        .await
        .expect("letting go of it twice must succeed");
    index
        .let_go_parcel(container_id(9), 0)
        .await
        .expect("letting go of a parcel never held must succeed");
    assert_eq!(
        index
            .held_parcels()
            .await
            .expect("reading the held parcels must succeed"),
        [parcel(1, 1), parcel(2, 0)],
    );
}
