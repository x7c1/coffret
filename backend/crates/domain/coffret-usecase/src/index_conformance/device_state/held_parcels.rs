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

/// The parcels of one Container are looked up by its ID, in parcel order, and
/// are the part of [`held_parcels`](crate::Index::held_parcels) that names it
/// (spec: PK-21).
///
/// What a page read asks of the parcels it may already have, and what letting
/// go of one Container's parcels asks: a catalog that answered with another
/// Container's parcel would have the read open bytes of the wrong Container,
/// and one that left one out would have it read a parcel again that this
/// device already keeps. A Container none of whose parcels is held, or one
/// whose last parcel was let go, has none.
pub async fn one_containers_held_parcels_are_looked_up_by_id(fixture: &IndexUnderTest) {
    let index = fixture.index();
    for held in [parcel(2, 0), parcel(1, 3), parcel(3, 1), parcel(1, 1)] {
        index
            .hold_parcel(held)
            .await
            .expect("recording a held parcel must succeed");
    }

    let of = |container: u8| async move {
        index
            .held_parcels_of(container_id(container))
            .await
            .expect("reading one Container's held parcels must succeed")
    };
    assert_eq!(
        of(1).await,
        [parcel(1, 1), parcel(1, 3)],
        "that Container's parcels alone, in parcel order",
    );
    assert_eq!(of(2).await, [parcel(2, 0)]);
    assert!(
        of(9).await.is_empty(),
        "a Container nothing is held of has none"
    );

    index
        .let_go_parcel(container_id(2), 0)
        .await
        .expect("letting go of a held parcel must succeed");
    assert!(of(2).await.is_empty(), "a parcel let go is held no longer");
    assert_eq!(
        of(1).await,
        index
            .held_parcels()
            .await
            .expect("reading the held parcels must succeed")
            .into_iter()
            .filter(|held| held.container_id == container_id(1))
            .collect::<Vec<_>>(),
        "the same rows the whole set holds for that Container",
    );
}
