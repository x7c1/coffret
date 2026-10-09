use std::ops::Range;
use std::sync::Mutex;

use coffret_model::Mtime;

use crate::entry_paths::entry_path;
use crate::fetch::{fetch_entry, fetch_folders, EntryFetch, EntryFetchOutcome, FetchError};
use crate::fetch_conformance::counting_store::CountingStore;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    container_handle, entry_at, entry_request, exists, filler, freeze_source, keys, map,
    parcel_count, plant, read, request, sync_source, write, Planted, OLDER,
};
use crate::fetch_conformance::parcels::{
    a_book, chunk_reads, fetch_page, held, map_the_root, ONE_PACK,
};

/// Every read a fetch makes of a Container's chunks is one whole parcel or the
/// whole object, and never anything shorter (spec: PK-16).
///
/// Pages that lie inside one parcel and pages that run across a boundary are
/// asked for, and then the folder is fetched whole, which reads a one-file
/// Container whole as well. Whatever was asked of either object past its
/// front is held against the parcel arithmetic of PK-19: no range starts or
/// ends where an Entry does.
pub async fn every_read_of_a_containers_chunks_is_a_whole_parcel_or_the_whole_object(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/cover.jpg",
        &filler(3_000, 0x77),
    );
    sync_source(fixture, &keys, 2).await;
    map_the_root(fixture).await;

    let counting = CountingStore::around(fixture.store());
    for (run, page) in [(3, 0), (4, 5), (5, 7)] {
        fetch_page(&counting, fixture, &keys, book.page(page), run).await;
    }
    fetch_folders(request(&counting, fixture, &keys, 6))
        .await
        .expect("fetching the rest of the Library must succeed");

    let parcels: Vec<Range<u64>> = (0..parcel_count(book.body_start, book.object_len))
        .map(|index| book.parcel(index))
        .collect();
    let asked = chunk_reads(&counting, &book);
    assert!(!asked.is_empty(), "the Pack's chunks were read at all");
    for range in &asked {
        assert!(
            parcels.contains(range),
            "{range:?} of the Pack is one whole parcel of {parcels:?}",
        );
    }

    let cover = entry_at(fixture.source(), "albums/cover.jpg")
        .await
        .container_id;
    let cover = container_handle(fixture.store(), cover).await;
    assert_eq!(
        counting.ranges_of(&cover),
        [None],
        "the one-file Container the folder fetch read was read whole, in one read",
    );
    assert_eq!(
        read(
            fixture.fs(),
            &fixture.target_folder().join("albums/cover.jpg")
        ),
        filler(3_000, 0x77),
    );
}

/// Fetching one page of a Pack of several volumes reads the parcels that page
/// overlaps and nothing else, places it, and places every other page those
/// parcels wholly cover — whichever volume it is in (spec: PK-16, PK-19).
pub async fn one_page_reads_its_parcels_and_places_what_they_cover(fixture: &FetchUnderTest) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;

    // Page 3 is the last of the first volume and lies wholly in parcel 1,
    // which also holds the first page of the second volume.
    let counting = CountingStore::around(fixture.store());
    let fetched = fetch_page(&counting, fixture, &keys, book.page(3), 2).await;
    assert_eq!(fetched.fetch, EntryFetch::Placed);

    assert_eq!(
        chunk_reads(&counting, &book),
        [book.parcel(1)],
        "the one parcel the page overlaps, and only it",
    );
    assert_eq!(
        &read(fixture.fs(), &book.placed(fixture, 3)),
        &book.pages[3].1
    );

    // Page 4 opens the second volume and lies wholly in parcel 1 too.
    assert_eq!(fetched.alongside, [entry_path(book.page(4))]);
    assert_eq!(
        &read(fixture.fs(), &book.placed(fixture, 4)),
        &book.pages[4].1
    );
    for outside in [0, 1, 2, 5, 6, 7] {
        assert!(
            !exists(fixture.fs(), &book.placed(fixture, outside)),
            "page {outside} reaches outside parcel 1 and was not placed",
        );
    }
}

/// The caller hears that the page it asked for is placed the moment the chunks
/// covering it have arrived, while the rest of its parcel is still on the way —
/// and the run itself still reads that parcel to its end (spec: PK-16, PK-21).
///
/// Page 3 lies wholly inside parcel 1 and page 4 follows it there, so at the
/// moment page 3 is published page 4's last bytes have not arrived: the hook
/// finds page 3 on disk and page 4 not yet, and is told of nothing placed
/// alongside. Once the run returns, page 4 is placed and parcel 1 is held.
pub async fn the_page_asked_for_is_published_before_its_parcel_has_arrived(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;

    let (asked, after) = (book.placed(fixture, 3), book.placed(fixture, 4));
    let heard: Mutex<Vec<(EntryFetchOutcome, bool, bool)>> = Mutex::new(Vec::new());
    let hook = |outcome: &EntryFetchOutcome| {
        heard
            .lock()
            .expect("the case's own lock is never poisoned")
            .push((
                outcome.clone(),
                exists(fixture.fs(), &asked),
                exists(fixture.fs(), &after),
            ));
    };
    let fetched = fetch_entry(
        entry_request(fixture.store(), fixture, &keys, book.page(3), 2).heard_by(&hook),
    )
    .await
    .expect("fetching page 3 must succeed");

    let heard = heard
        .into_inner()
        .expect("the case's own lock is never poisoned");
    let [(published, asked_there, after_there)] = heard.as_slice() else {
        panic!("the hook is told once, and was told {} times", heard.len());
    };
    assert_eq!(published.fetch, EntryFetch::Placed);
    assert!(
        *asked_there,
        "the page asked for is on disk when the hook hears of it"
    );
    assert!(
        !*after_there,
        "and the page after it in the same parcel is not yet: the parcel was still arriving",
    );
    assert!(
        published.alongside.is_empty(),
        "nothing of the parcel had been placed besides it yet",
    );

    assert_eq!(fetched.fetch, EntryFetch::Placed);
    assert_eq!(
        fetched.alongside,
        [entry_path(book.page(4))],
        "the run went on to the parcel's end and placed what it covers",
    );
    assert_eq!(&read(fixture.fs(), &after), &book.pages[4].1);
    assert_eq!(
        held(fixture, book.container_id).await,
        [1],
        "and the parcel, read whole, is held (spec: PK-21)",
    );
}

/// A page that runs across a parcel boundary is read from both parcels
/// (spec: PK-16).
pub async fn an_entry_across_a_parcel_boundary_reads_both_parcels(fixture: &FetchUnderTest) {
    let keys = keys();
    let book = a_book(fixture, &keys, ["books/atlas/vol1", "books/atlas/vol2"]).await;
    map_the_root(fixture).await;

    let counting = CountingStore::around(fixture.store());
    let fetched = fetch_page(&counting, fixture, &keys, book.page(2), 2).await;
    assert_eq!(fetched.fetch, EntryFetch::Placed);

    assert_eq!(
        chunk_reads(&counting, &book),
        [book.parcel(0), book.parcel(1)],
        "both parcels the page overlaps, each whole and each once",
    );
    assert_eq!(
        &read(fixture.fs(), &book.placed(fixture, 2)),
        &book.pages[2].1
    );
    assert_eq!(
        fetched.alongside,
        [0, 1, 3, 4].map(|page| entry_path(book.page(page))),
        "every page the two parcels wholly cover came along, in stream order",
    );
}

/// An Entry taken out of parcels is held to chunk authentication and to its
/// plaintext hash, not to the Container's ciphertext hash; a read of the whole
/// object still is (spec: PK-22).
///
/// Both Containers are committed under a ciphertext hash that is not what is
/// stored. The page read by its parcels is placed, because nothing it read is
/// in question; the folder fetch, which reads the other Container whole, holds
/// it to that hash and refuses it.
pub async fn a_parcel_read_is_not_held_to_the_ciphertext_hash_and_a_whole_read_is(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    map_the_root(fixture).await;
    let mut planted = Vec::new();
    for (path, content) in [
        ("a.jpg", b"the page read by its parcels".as_slice()),
        ("b.jpg", b"the page read with its whole object".as_slice()),
    ] {
        planted.push(
            plant(
                fixture.store(),
                fixture.source(),
                &keys,
                Planted {
                    path,
                    content,
                    mtime: Mtime::from_unix_seconds(OLDER),
                    real: true,
                    actual_content: None,
                    meta_len: None,
                    short_by: None,
                    misrecorded: true,
                },
            )
            .await,
        );
    }

    let fetched = fetch_page(fixture.store(), fixture, &keys, "a.jpg", 2).await;
    assert_eq!(fetched.fetch, EntryFetch::Placed);
    assert_eq!(
        read(fixture.fs(), &fixture.target_folder().join("a.jpg")),
        b"the page read by its parcels",
    );

    let result = fetch_folders(request(fixture.store(), fixture, &keys, 3)).await;
    let Err(FetchError::CiphertextMismatch { container_id, .. }) = result else {
        panic!("expected the whole read to be held to the recorded hash, got {result:?}");
    };
    assert_eq!(container_id, planted[1]);
    assert!(!exists(
        fixture.fs(),
        &fixture.target_folder().join("b.jpg")
    ));
}

/// An Entry of no bytes is placed by the parcel its position stands in, like
/// any other: it names that parcel, and is complete the moment the read
/// reaches it (spec: PK-16, FM-4).
pub async fn an_entry_of_no_bytes_is_placed_by_its_parcel(fixture: &FetchUnderTest) {
    let keys = keys();
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    map_the_root(fixture).await;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "notes/a.txt",
        b"before",
    );
    write(fixture.fs(), fixture.source_folder(), "notes/z.txt", b"");
    freeze_source(fixture, &keys, ONE_PACK, 1).await;

    let fetched = fetch_page(fixture.store(), fixture, &keys, "notes/z.txt", 2).await;
    assert_eq!(fetched.fetch, EntryFetch::Placed);
    assert_eq!(
        read(fixture.fs(), &fixture.target_folder().join("notes/z.txt")),
        b"",
    );
    assert_eq!(fetched.alongside, [entry_path("notes/a.txt")]);
}
