use std::collections::BTreeSet;
use std::sync::Arc;

use coffret_model::{ContainerKind, EntryPath};

use crate::device_state::{LiveReservations, ReservationId};
use crate::entry_paths::entry_path;
use crate::freeze::{freeze_folder, Lapse, Lapsed};
use crate::freeze_conformance::fixtures::{
    filler, freeze_only, keys, map, pending, request, spooled, sync_source, write, TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;
use crate::sync::{Disposal, Settled};

/// The pages of the book these cases drop, under `scans/vol-1`.
const PAGES: [&str; 4] = [
    "scans/vol-1/page-001.jpg",
    "scans/vol-1/page-002.jpg",
    "scans/vol-1/page-003.jpg",
    "scans/vol-1/page-004.jpg",
];

/// Lands the book's pages in the source folder, the way a drop writes them:
/// each one reserved before it lands (spec: PK-17).
async fn dropped(fixture: &FreezeUnderTest) -> ReservationId {
    let index = fixture.source();
    let reservation = index
        .reserve(Some(&entry_path("scans")))
        .await
        .expect("recording a reservation must succeed");
    for (number, path) in PAGES.iter().enumerate() {
        index
            .extend_reservation(reservation, &entry_path(*path))
            .await
            .expect("extending a reservation must succeed");
        write(
            fixture.fs(),
            fixture.source_folder(),
            path,
            &filler(60 + number * 11, 0x40 + number as u8),
        );
    }
    reservation
}

fn paths(paths: &[&str]) -> BTreeSet<EntryPath> {
    paths.iter().map(|path| entry_path(*path)).collect()
}

/// A sync run while a drop's files wait for their Pack leaves them alone and
/// says so, and the freeze that takes the reservation up afterwards packs
/// exactly those files into Packs, with no Container of their own made first
/// (spec: PK-17, PK-7).
///
/// This is the shape a restart produces: the process that took the drop is
/// gone, a sync another drop armed walks the mapped folder first, and the
/// reservation is the only thing that tells the pages from loose files.
pub async fn a_sync_leaves_reserved_files_for_their_pack(fixture: &FreezeUnderTest) {
    let keys = keys();
    let index = fixture.source();
    map(index, None, fixture.source_folder()).await;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/loose.jpg",
        b"a photo dropped on its own",
    );
    dropped(fixture).await;

    let synced = sync_source(fixture, &keys, 1).await;
    assert_eq!(
        synced.added.len(),
        1,
        "the loose photo is carried in and the reserved pages are not: {synced:?}",
    );
    assert_eq!(
        synced.reserved.iter().cloned().collect::<BTreeSet<_>>(),
        paths(&PAGES),
        "and every page is reported as waiting for its Pack rather than passed over",
    );
    for page in PAGES {
        assert!(
            index
                .entry_at(&entry_path(page))
                .await
                .expect("reading an Entry must succeed")
                .is_none(),
            "{page} is not an Entry of its own",
        );
    }

    let frozen = freeze_only(fixture, &keys, "scans", &PAGES, TARGET, 2).await;
    assert_eq!(frozen.frozen_entries(), PAGES.len());
    assert_eq!(frozen.released, 1, "the commit releases the reservation");
    assert_eq!(frozen.lapsed, Vec::new());
    let commit = frozen.commit.expect("the pages are committed");
    assert!(
        commit
            .record
            .additions()
            .iter()
            .all(|addition| addition.container().kind == ContainerKind::Pack),
        "every page went up in a Pack",
    );
    assert!(
        commit.record.removals().is_empty(),
        "and none of them had a Container of its own to absorb",
    );
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        Vec::new(),
    );

    let again = sync_source(fixture, &keys, 3).await;
    assert!(
        again.reserved.is_empty() && again.added.is_empty(),
        "once packed, the pages are ordinary Entries to a sync: {again:?}",
    );
}

/// A reservation whose files have all gone is released with a finding per
/// file saying why, and nothing is committed for it; one whose files are
/// partly gone packs the rest and is released by the commit (spec: PK-17).
pub async fn a_reservation_is_released_whether_or_not_its_files_remain(fixture: &FreezeUnderTest) {
    let keys = keys();
    let index = fixture.source();
    map(index, None, fixture.source_folder()).await;
    let partly = dropped(fixture).await;
    fixture
        .fs()
        .remove_file(&fixture.source_folder().join(PAGES[1]));

    let gone = index
        .reserve(Some(&entry_path("scans")))
        .await
        .expect("recording a reservation must succeed");
    for path in ["scans/vol-2/page-001.jpg", "scans/vol-2/page-002.jpg"] {
        index
            .extend_reservation(gone, &entry_path(path))
            .await
            .expect("extending a reservation must succeed");
    }

    let frozen = freeze_folder(request(
        fixture.store(),
        index,
        &keys,
        fixture.fs(),
        TARGET,
        1,
    ))
    .await
    .expect("a freeze of the whole Library must succeed");

    assert_eq!(
        frozen.frozen_entries(),
        PAGES.len() - 1,
        "the pages that remain are packed, and the one deleted is not",
    );
    assert_eq!(frozen.released, 2, "both reservations are decided");
    assert_eq!(
        frozen.lapsed,
        vec![
            Lapsed {
                path: entry_path("scans/vol-2/page-001.jpg"),
                lapse: Lapse::Gone,
            },
            Lapsed {
                path: entry_path("scans/vol-2/page-002.jpg"),
                lapse: Lapse::Gone,
            },
        ],
        "the reservation none of whose files is left says why for each of them, and the \
         one that packed something says nothing about the page it lost",
    );
    assert!(frozen.commit.is_some());
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        Vec::new(),
        "{partly} and {gone} are both gone",
    );
}

/// A reservation naming no path that no drop holds live — one the Index has
/// just drawn, a moment before the drop holds it, or a drop that stopped
/// before its first page — is left alone by a freeze of anything else, and
/// released by the run a server takes it up as: its folder, with a selection
/// of nothing (spec: PK-17).
///
/// Every path of such a reservation is trivially one a run considered, so
/// without the distinction a freeze of an earlier book running in that moment
/// would release the next drop's reservation before its first page lands, and
/// those pages would land with nothing keeping a sync from carrying them in
/// one Container each.
pub async fn a_reservation_naming_no_path_is_released_only_by_its_own_run(
    fixture: &FreezeUnderTest,
) {
    let keys = keys();
    let index = fixture.source();
    map(index, None, fixture.source_folder()).await;
    dropped(fixture).await;
    let under_way = index
        .reserve(Some(&entry_path("scans")))
        .await
        .expect("recording a reservation must succeed");

    let frozen = freeze_only(fixture, &keys, "scans/vol-1", &PAGES, TARGET, 1).await;
    assert_eq!(frozen.frozen_entries(), PAGES.len());
    assert_eq!(
        frozen.released, 1,
        "only the packed book's reservation goes"
    );
    let left: Vec<ReservationId> = index
        .reservations()
        .await
        .expect("reading the reservations must succeed")
        .into_iter()
        .map(|reservation| reservation.id)
        .collect();
    assert_eq!(left, vec![under_way], "the drop under way keeps its own");

    let resumed = freeze_only(fixture, &keys, "scans", &[], TARGET, 2).await;
    assert_eq!(resumed.released, 1, "its own run releases it");
    assert_eq!(
        resumed.lapsed,
        Vec::new(),
        "with no path to say anything of"
    );
    assert!(resumed.commit.is_none());
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        Vec::new(),
    );
}

/// Lands `pages` of the book under the reservation `id`, as a drop still
/// receiving does.
async fn landing(fixture: &FreezeUnderTest, id: ReservationId, pages: &[&str]) {
    for path in pages {
        let number = PAGES.iter().position(|page| page == path).unwrap_or(0);
        fixture
            .source()
            .extend_reservation(id, &entry_path(*path))
            .await
            .expect("extending a reservation must succeed");
        write(
            fixture.fs(),
            fixture.source_folder(),
            path,
            &filler(60 + number * 11, 0x40 + number as u8),
        );
    }
}

/// A freeze of the whole folder running while a drop into it is still
/// receiving packs the pages landed so far but decides nothing of the drop's
/// reservation, and the drop's own run, once it stops receiving, packs the
/// pages that landed after and releases it (spec: PK-17).
///
/// Releasing it from the earlier run would leave the later pages landing
/// unreserved, for a sync after a restart to carry in one Container each.
pub async fn a_freeze_beside_a_drop_still_landing_leaves_its_reservation(
    fixture: &FreezeUnderTest,
) {
    let keys = keys();
    let index = fixture.source();
    map(index, None, fixture.source_folder()).await;
    let live = Arc::new(LiveReservations::new());
    let id = index
        .reserve(Some(&entry_path("scans")))
        .await
        .expect("recording a reservation must succeed");
    let held = live.hold(id);
    landing(fixture, id, &PAGES[..2]).await;

    let beside = freeze_folder(
        request(fixture.store(), index, &keys, fixture.fs(), TARGET, 1)
            .under(entry_path("scans"))
            .sparing(&live),
    )
    .await
    .expect("a freeze of the folder must succeed");
    assert_eq!(
        beside.frozen_entries(),
        2,
        "the pages landed so far are packed"
    );
    assert_eq!(beside.released, 0, "and the reservation is not released");
    assert_eq!(beside.lapsed, Vec::new());

    landing(fixture, id, &PAGES[2..]).await;
    drop(held);
    let own = freeze_folder(
        request(fixture.store(), index, &keys, fixture.fs(), TARGET, 2)
            .under(entry_path("scans/vol-1"))
            .only(paths(&PAGES))
            .sparing(&live),
    )
    .await
    .expect("the drop's own freeze must succeed");
    assert_eq!(
        own.frozen_entries(),
        2,
        "the drop's own run packs the pages that landed after",
    );
    assert_eq!(own.released, 1, "and releases the reservation");
    assert_eq!(
        own.lapsed,
        Vec::new(),
        "the pages packed beside it are not findings"
    );
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        Vec::new(),
    );
}

/// A reservation that was live in a process that has since ended is one the
/// next process's freeze releases: nothing is live after a restart, so it is
/// taken up like any other (spec: PK-17, LA-12).
pub async fn a_reservation_live_before_a_restart_is_released_after_it(fixture: &FreezeUnderTest) {
    let keys = keys();
    let index = fixture.source();
    map(index, None, fixture.source_folder()).await;
    let before = Arc::new(LiveReservations::new());
    let id = index
        .reserve(Some(&entry_path("scans")))
        .await
        .expect("recording a reservation must succeed");
    // What the process that took the drop held, still holding it as it was
    // stopped. Nothing of it reaches the next process, which starts holding
    // nothing.
    let _held = before.hold(id);
    landing(fixture, id, &PAGES[..3]).await;

    let after = Arc::new(LiveReservations::new());
    assert!(before.is_live(id) && !after.is_live(id));
    let resumed = freeze_folder(
        request(fixture.store(), index, &keys, fixture.fs(), TARGET, 2)
            .under(entry_path("scans/vol-1"))
            .only(paths(&PAGES[..3]))
            .sparing(&after),
    )
    .await
    .expect("the freeze after the restart must succeed");
    assert_eq!(resumed.frozen_entries(), 3, "the pages it named are packed");
    assert_eq!(resumed.released, 1, "and the next process releases it");
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        Vec::new(),
    );
}

/// A freeze settles what an interrupted run left before it builds anything of
/// its own: the spool and the row of a Pack a dead run never finished are
/// disposed of, and the files are packed afresh (spec: OC-9, OC-2, OC-3).
///
/// Without it a Pack-only workflow would keep the dead run's spool — up to one
/// Pack's size on disk — and its row until a sync happened to run.
pub async fn a_freeze_settles_an_interrupted_spool_before_it_builds(fixture: &FreezeUnderTest) {
    let keys = keys();
    let index = fixture.source();
    let abandoned = super::interruption::interrupted_spool(fixture, &keys).await;

    let frozen = freeze_folder(request(
        fixture.store(),
        index,
        &keys,
        fixture.fs(),
        TARGET,
        2,
    ))
    .await
    .expect("the freeze after the interrupted one must succeed");

    assert!(
        matches!(
            &frozen.settled[..],
            [Settled::Disposed {
                container_id: settled,
                disposal: Disposal::NeverUploaded,
            }] if *settled == abandoned
        ),
        "the dead run's Pack is this device's own to reclaim (spec: OC-2): {:?}",
        frozen.settled,
    );
    let commit = frozen.commit.expect("the files are packed afresh");
    assert!(
        !commit
            .record
            .additions()
            .iter()
            .any(|addition| addition.container().id == abandoned),
        "the abandoned Pack is not resumed (spec: KD-2)",
    );
    assert!(
        pending(index).await.is_empty(),
        "no row is left behind, the dead run's or this one's",
    );
    assert_eq!(
        spooled(fixture.fs()),
        0,
        "and no spool either: the abandoned one was disposed of before this run built its own",
    );
}
