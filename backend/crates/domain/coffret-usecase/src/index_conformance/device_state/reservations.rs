use std::collections::BTreeSet;

use coffret_model::{ContainerKind, EntryPath};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{Reservation, ReservationId};
use crate::entry_paths::entry_path;
use crate::index::Index;
use crate::index_conformance::fixtures::{addition, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// The paths a reservation names, spelled as literals.
fn paths(paths: &[&str]) -> BTreeSet<EntryPath> {
    paths.iter().map(|path| entry_path(*path)).collect()
}

/// Makes a reservation of `folder` naming `named`, the way a drop does: the
/// reservation first, then one path at a time.
async fn reserved(index: &dyn Index, folder: Option<&str>, named: &[&str]) -> ReservationId {
    let folder = folder.map(entry_path);
    let id = index
        .reserve(folder.as_ref())
        .await
        .expect("recording a reservation must succeed");
    for path in named {
        index
            .extend_reservation(id, &entry_path(*path))
            .await
            .expect("extending a reservation must succeed");
    }
    id
}

/// A reservation is kept, in the order it was made and with every path it was
/// extended by, until it is released; releasing one twice succeeds, and
/// extending one that is gone makes none up (spec: PK-17, OC-8).
///
/// The order is what a server resumes them in, so a catalog must answer with
/// the reservation made first first, whatever folders they name — the Library
/// root among them.
pub async fn a_reservation_is_kept_in_order_until_it_is_released(fixture: &IndexUnderTest) {
    let index = fixture.index();
    let first = reserved(
        index,
        Some("scans"),
        &["scans/vol-1/b.jpg", "scans/vol-1/a.jpg"],
    )
    .await;
    let second = reserved(index, None, &["loose.jpg"]).await;
    assert!(
        first < second,
        "a later reservation is drawn a greater identifier"
    );
    index
        .extend_reservation(first, &entry_path("scans/vol-1/a.jpg"))
        .await
        .expect("extending a reservation by a path it names must succeed");

    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        [
            Reservation {
                id: first,
                folder: Some(entry_path("scans")),
                paths: paths(&["scans/vol-1/a.jpg", "scans/vol-1/b.jpg"]),
            },
            Reservation {
                id: second,
                folder: None,
                paths: paths(&["loose.jpg"]),
            },
        ],
        "every reservation, in the order it was made, each path once",
    );

    index
        .release_reservation(first)
        .await
        .expect("releasing a reservation must succeed");
    index
        .release_reservation(first)
        .await
        .expect("releasing it twice must succeed");
    index
        .extend_reservation(first, &entry_path("scans/vol-1/c.jpg"))
        .await
        .expect("extending a reservation that is gone changes nothing and succeeds");
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        [Reservation {
            id: second,
            folder: None,
            paths: paths(&["loose.jpg"]),
        }],
    );

    let third = reserved(index, Some("books"), &[]).await;
    assert!(
        third > second,
        "an identifier is never drawn twice, even once the one before it is gone",
    );
}

/// The refresh of a batch that packs a reservation releases it in the same
/// operation, and leaves every other reservation where it was (spec: PK-17).
pub async fn a_refresh_releases_the_reservations_its_batch_packs(fixture: &IndexUnderTest) {
    let index = fixture.index();
    let packed = reserved(index, Some("albums"), &["albums/a.jpg"]).await;
    let waiting = reserved(index, Some("books"), &["books/page-001.jpg"]).await;

    index
        .refresh(CommittedBatch {
            record: record(
                0,
                vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
                vec![],
            ),
            materialized: Vec::new(),
            released: vec![packed],
        })
        .await
        .expect("refreshing after a commit must succeed");

    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed")
            .into_iter()
            .map(|reservation| reservation.id)
            .collect::<Vec<_>>(),
        [waiting],
        "the batch packed one reservation, and only that one went with it",
    );
}
