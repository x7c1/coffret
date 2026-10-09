use std::path::PathBuf;

use crate::catch_up::{catch_up_catalog, CatchUpRequest};
use crate::delete::{delete_entries, DeleteRequest, DeleteSelection};
use crate::device_state::BatchId;
use crate::entry_paths::entry_path;
use crate::fetch::{let_go_parcels, EntryFetch, UnheldParcel, UnheldReason};
use crate::fetch_conformance::counting_store::CountingStore;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{at, entry_at, exists, keys, map, policy, read};
use crate::fetch_conformance::parcels::{
    a_book, chunk_reads, fetch_page, held, held_rows, map_the_root, PAGES_PER_VOLUME,
};

/// A page read a second time, and a neighbouring page inside a parcel the
/// device holds, read nothing from Storage; a page needing a held parcel and
/// one more reads only the one more (spec: PK-21).
pub async fn a_revisit_of_a_held_parcel_reads_nothing_from_storage(fixture: &FetchUnderTest) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;

    let first = CountingStore::around(fixture.store());
    fetch_page(&first, fixture, &keys, book.page(2), 2).await;
    assert_eq!(held(fixture, book.container_id).await, [1]);

    // The same page again, and page 3 inside the parcel the device holds.
    let again = CountingStore::around(fixture.store());
    for (run, page) in [(3, 2), (4, 3)] {
        let fetched = fetch_page(&again, fixture, &keys, book.page(page), run).await;
        assert_eq!(fetched.fetch, EntryFetch::AlreadyPresent);
    }
    assert!(
        again.ranges_of(&book.object).is_empty(),
        "a revisit asks Storage nothing of the Pack: {:?}",
        again.ranges_of(&book.object),
    );

    // Page 5 needs parcel 1, which the device holds, and parcel 2, which it
    // does not: only parcel 2 is asked for.
    let further = CountingStore::around(fixture.store());
    let fetched = fetch_page(&further, fixture, &keys, book.page(5), 5).await;
    assert_eq!(fetched.fetch, EntryFetch::Placed);
    assert_eq!(chunk_reads(&further, &book), [book.parcel(2)]);
    assert_eq!(
        &read(fixture.fs(), &book.placed(fixture, 5)),
        &book.pages[5].1
    );
}

/// A parcel is kept while an Entry it covers is not on the device, and let go,
/// file and row together, once every one of them is (spec: PK-21).
pub async fn a_parcel_is_let_go_once_every_entry_it_covers_is_present(fixture: &FetchUnderTest) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;

    // Pages 0 to 4 are placed; parcel 0 has nothing left to wait for, and
    // parcel 1 waits for page 5.
    fetch_page(fixture.store(), fixture, &keys, book.page(2), 2).await;
    assert_eq!(held(fixture, book.container_id).await, [1]);
    let kept = held_rows(fixture, book.container_id).await;
    assert_eq!(
        fixture.fs().files_beneath(fixture.parcel_dir()),
        [kept[0].path.clone()],
        "the one parcel held is the one file kept",
    );

    // Page 5 completes parcel 1 and brings page 6 along; parcel 2 waits for
    // page 7.
    fetch_page(fixture.store(), fixture, &keys, book.page(5), 3).await;
    assert_eq!(held(fixture, book.container_id).await, [2]);

    // Page 7 completes the Pack, and nothing is held of it any more.
    let counting = CountingStore::around(fixture.store());
    fetch_page(&counting, fixture, &keys, book.page(7), 4).await;
    assert_eq!(chunk_reads(&counting, &book), [book.parcel(3)]);
    assert!(held(fixture, book.container_id).await.is_empty());
    assert!(
        fixture.fs().files_beneath(fixture.parcel_dir()).is_empty(),
        "every parcel file went with its row",
    );
    for (index, (_, content)) in book.pages.iter().enumerate() {
        assert_eq!(&read(fixture.fs(), &book.placed(fixture, index)), content);
    }
}

/// An Entry this device does not map holds no parcel: a device mapping one
/// volume lets go of a parcel it shares with the other once its own pages in
/// it are placed (spec: PK-21, EP-9).
pub async fn a_parcel_is_not_kept_for_an_entry_this_device_does_not_map(fixture: &FetchUnderTest) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["vol1", "vol2"]).await;
    // Only the first volume is on this device.
    map(
        fixture.target(),
        fixture.fs(),
        Some("vol1"),
        fixture.target_folder(),
    )
    .await;

    // Page 3 lies in parcel 1, beside page 2 (which also needs parcel 0) and
    // the second volume's first pages: parcel 1 waits for page 2.
    fetch_page(fixture.store(), fixture, &keys, book.page(3), 2).await;
    assert_eq!(held(fixture, book.container_id).await, [1]);
    assert!(
        !exists(fixture.fs(), &fixture.target_folder().join("004.jpg")),
        "a page of the volume this device does not map is not placed",
    );

    // Page 2 is the last page of this device's that parcel 1 covers. What is
    // left in it is the other volume, which this device never waits for.
    let counting = CountingStore::around(fixture.store());
    fetch_page(&counting, fixture, &keys, book.page(2), 3).await;
    assert_eq!(chunk_reads(&counting, &book), [book.parcel(0)]);
    assert!(
        held(fixture, book.container_id).await.is_empty(),
        "nothing is held for the volume this device does not map",
    );
    for page in 0..PAGES_PER_VOLUME {
        assert_eq!(
            read(
                fixture.fs(),
                &fixture.target_folder().join(format!("{page:03}.jpg")),
            ),
            book.pages[page].1,
        );
    }
}

/// A parcel of a Container that left the current set is let go, file and row
/// together, by the next run that catches up (spec: PK-21, CK-9).
pub async fn a_parcel_of_a_container_that_left_the_current_set_is_let_go(fixture: &FetchUnderTest) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;
    fetch_page(fixture.store(), fixture, &keys, book.page(2), 2).await;
    assert_eq!(held(fixture, book.container_id).await, [1]);

    // The source device deletes the last page, which rebuilds the Pack around
    // the rest under a new Container ID (spec: PK-10).
    delete_entries(
        DeleteRequest::new(
            fixture.store(),
            fixture.source(),
            &keys,
            fixture.fs(),
            fixture.spool_dir(),
            DeleteSelection::paths([entry_path(book.page(7))].into()),
            BatchId::new("delete-3"),
            at(3),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("deleting a page must succeed: {error}"));
    assert_ne!(
        entry_at(fixture.source(), book.page(6)).await.container_id,
        book.container_id,
        "the Pack was replaced",
    );

    // Any run that catches up lets the old Pack's parcels go.
    fetch_page(fixture.store(), fixture, &keys, book.page(6), 4).await;
    assert!(held(fixture, book.container_id).await.is_empty());
    let left: Vec<PathBuf> = fixture.fs().files_beneath(fixture.parcel_dir());
    assert!(
        left.iter().all(|file| !file
            .to_string_lossy()
            .contains(&book.container_id.to_string())),
        "no file of the replaced Pack is left: {left:?}",
    );
}

/// A parcel is let go once every Entry it covers is present or witnessed
/// absent: a page this device placed and then saw go is not waited for, and is
/// not put back by the parcels read for its neighbour (spec: PK-21, EP-10,
/// EP-11).
pub async fn a_parcel_is_let_go_once_the_entries_it_waits_for_are_witnessed_absent(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;

    // Page 5 reads parcels 1 and 2 and brings pages 3, 4 and 6 along; parcel 1
    // waits for page 2, and parcel 2 for page 7.
    fetch_page(fixture.store(), fixture, &keys, book.page(5), 2).await;
    assert_eq!(held(fixture, book.container_id).await, [1, 2]);

    // The person removes page 5, and a scan witnesses it gone (spec: EP-10).
    fixture.fs().remove_file(&book.placed(fixture, 5));
    fixture
        .target()
        .mark_absent(&entry_path(book.page(5)), at(3))
        .await
        .expect("recording a witnessed deletion must succeed");

    // Page 2 reads parcel 0 and takes the rest of itself out of the held parcel
    // 1, whose Entries are now all present but page 5, which is witnessed
    // absent: parcel 1 goes, and parcel 2 still waits for page 7.
    let counting = CountingStore::around(fixture.store());
    let fetched = fetch_page(&counting, fixture, &keys, book.page(2), 4).await;
    assert_eq!(fetched.fetch, EntryFetch::Placed);
    assert_eq!(chunk_reads(&counting, &book), [book.parcel(0)]);
    assert_eq!(held(fixture, book.container_id).await, [2]);
    assert!(
        !exists(fixture.fs(), &book.placed(fixture, 5)),
        "a page witnessed gone is not put back by its neighbour's parcels",
    );
}

/// A catch-up followed by [`let_go_parcels`] — what a device runs after a
/// catch-up or a deletion of its own, neither of which reads a parcel — lets go
/// of the parcels of a Container that left the current set, file and row
/// together (spec: PK-21, CK-9).
pub async fn letting_go_after_a_catch_up_drops_the_parcels_of_a_departed_container(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;
    fetch_page(fixture.store(), fixture, &keys, book.page(2), 2).await;
    assert_eq!(held(fixture, book.container_id).await, [1]);

    // The source device deletes the last page, which rebuilds the Pack around
    // the rest under a new Container ID (spec: PK-10).
    delete_entries(
        DeleteRequest::new(
            fixture.store(),
            fixture.source(),
            &keys,
            fixture.fs(),
            fixture.spool_dir(),
            DeleteSelection::paths([entry_path(book.page(7))].into()),
            BatchId::new("delete-3"),
            at(3),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("deleting a page must succeed: {error}"));

    // The catch-up on its own moves the catalog and nothing on the disk.
    catch_up_catalog(CatchUpRequest {
        policy: policy(),
        ..CatchUpRequest::new(fixture.store(), fixture.target(), &keys)
    })
    .await
    .unwrap_or_else(|error| panic!("catching the target up must succeed: {error}"));
    assert_eq!(held(fixture, book.container_id).await, [1]);

    let_go_parcels(fixture.target(), &fixture.parcels())
        .await
        .unwrap_or_else(|error| panic!("letting go of parcels must succeed: {error}"));
    assert!(held(fixture, book.container_id).await.is_empty());
    assert!(
        fixture.fs().files_beneath(fixture.parcel_dir()).is_empty(),
        "the replaced Pack's parcel file went with its row",
    );
}

/// A kept parcel whose file is gone, or no longer authenticates, is not held:
/// it is said, let go, and read from Storage again
/// (spec: PK-21, FM-5).
pub async fn a_kept_parcel_that_is_gone_or_damaged_is_read_again_with_a_finding(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;
    fetch_page(fixture.store(), fixture, &keys, book.page(2), 2).await;
    let kept = held_rows(fixture, book.container_id).await;
    assert_eq!(kept.len(), 1);

    // Parcel 1's file, damaged in place.
    fixture
        .fs()
        .write_file(&kept[0].path, b"not the parcel this device read");
    let counting = CountingStore::around(fixture.store());
    let fetched = fetch_page(&counting, fixture, &keys, book.page(5), 3).await;
    assert_eq!(fetched.fetch, EntryFetch::Placed);
    assert!(
        matches!(
            fetched.unheld.as_slice(),
            [UnheldParcel {
                container_id,
                index: 1,
                reason: UnheldReason::Unauthenticated { .. },
            }] if *container_id == book.container_id
        ),
        "the damaged parcel was said as not authenticating: {:?}",
        fetched.unheld,
    );
    assert_eq!(
        chunk_reads(&counting, &book),
        [book.parcel(1), book.parcel(2)],
        "the damaged parcel was read again, whole, and the next one beside it",
    );
    assert_eq!(
        &read(fixture.fs(), &book.placed(fixture, 5)),
        &book.pages[5].1
    );

    // Parcel 2's file, gone.
    let kept = held_rows(fixture, book.container_id).await;
    assert_eq!(
        kept.iter().map(|parcel| parcel.index).collect::<Vec<_>>(),
        [2]
    );
    fixture.fs().remove_file(&kept[0].path);
    let counting = CountingStore::around(fixture.store());
    let fetched = fetch_page(&counting, fixture, &keys, book.page(7), 4).await;
    assert!(
        matches!(
            fetched.unheld.as_slice(),
            [UnheldParcel {
                container_id,
                index: 2,
                reason: UnheldReason::Missing,
            }] if *container_id == book.container_id
        ),
        "the vanished parcel was said as missing: {:?}",
        fetched.unheld,
    );
    assert_eq!(
        chunk_reads(&counting, &book),
        [book.parcel(2), book.parcel(3)]
    );
    assert_eq!(
        &read(fixture.fs(), &book.placed(fixture, 7)),
        &book.pages[7].1
    );
    assert!(held(fixture, book.container_id).await.is_empty());
}
