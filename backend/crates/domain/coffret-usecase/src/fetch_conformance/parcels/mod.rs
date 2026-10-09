//! What a fetch reads of a Container's chunks, and what it keeps of them.
//!
//! The fetch unit is the parcel (spec: PK-16, PK-19), so every case here is
//! about which byte ranges the fetching device asked Storage for — which
//! nothing a fetch returns can carry, so the reads are counted — and about which
//! parcels it holds afterwards (spec: PK-21). The suite reads at one chunk per
//! parcel (see `suite_parcel_len`), so the Pack these cases build out of a few
//! mebibytes is several parcels long.

use std::ops::Range;
use std::path::PathBuf;

use coffret_model::{ContainerId, ObjectRef};

use crate::device_state::HeldParcel;
use crate::fetch::{fetch_entry, EntryFetchOutcome, LibraryKeys};
use crate::fetch_conformance::counting_store::CountingStore;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    body_start, container_handle, entry_at, entry_request, filler, freeze_source, map,
    parcel_count, parcel_range, parcels_overlapping, write,
};
use crate::object_store::ObjectStore;

mod keeping;
pub use keeping::{
    a_kept_parcel_that_is_gone_or_damaged_is_read_again_with_a_finding,
    a_parcel_is_let_go_once_every_entry_it_covers_is_present,
    a_parcel_is_let_go_once_the_entries_it_waits_for_are_witnessed_absent,
    a_parcel_is_not_kept_for_an_entry_this_device_does_not_map,
    a_parcel_of_a_container_that_left_the_current_set_is_let_go,
    a_revisit_of_a_held_parcel_reads_nothing_from_storage,
    letting_go_after_a_catch_up_drops_the_parcels_of_a_departed_container,
};

mod reading;
pub use reading::{
    a_parcel_read_is_not_held_to_the_ciphertext_hash_and_a_whole_read_is,
    an_entry_across_a_parcel_boundary_reads_both_parcels,
    an_entry_of_no_bytes_is_placed_by_its_parcel,
    every_read_of_a_containers_chunks_is_a_whole_parcel_or_the_whole_object,
    one_page_reads_its_parcels_and_places_what_they_cover,
    the_page_asked_for_is_published_before_its_parcel_has_arrived,
};

/// How long each page of the book the cases build is.
///
/// At 400 KiB a page against 1 MiB parcels, the eight pages land so that some
/// lie wholly inside one parcel and some run across a boundary — the layout
/// every case here needs both kinds of.
const PAGE_LEN: usize = 400 * 1024;

/// How many pages each of the book's two volumes has.
const PAGES_PER_VOLUME: usize = 4;

/// A size target roomy enough that both volumes land in one Pack (spec: PK-5).
const ONE_PACK: u64 = 16 * 1024 * 1024;

/// One book of two volumes, frozen into one Pack on the source device.
struct Book {
    /// Every page, in the order the Pack holds them: its Entry Path relative to
    /// the source folder, and its content.
    pages: Vec<(String, Vec<u8>)>,
    /// The Pack's object.
    object: ObjectRef,
    /// The Pack's Container ID.
    container_id: ContainerId,
    /// Where its chunk sequence starts (spec: FM-2).
    body_start: u64,
    /// How long the whole object is.
    object_len: u64,
}

impl Book {
    /// The page at `index`, as its Entry Path.
    fn page(&self, index: usize) -> &str {
        &self.pages[index].0
    }

    /// The parcels page `index` overlaps (spec: PK-19).
    async fn parcels_of(&self, fixture: &FetchUnderTest, index: usize) -> Range<u64> {
        parcels_overlapping(
            &entry_at(fixture.source(), self.page(index))
                .await
                .entry
                .extent,
        )
    }

    /// The object range parcel `index` occupies (spec: PK-16).
    fn parcel(&self, index: u64) -> Range<u64> {
        parcel_range(self.body_start, self.object_len, index)
    }

    /// Where on the target device page `index` is placed, under a mapping of
    /// the Library root.
    fn placed(&self, fixture: &FetchUnderTest, index: usize) -> PathBuf {
        fixture.target_folder().join(self.page(index))
    }
}

/// Freezes a book of two volumes, one folder each, into one Pack on the source
/// device.
///
/// The folders are the case's to name: two folders of one book, or two
/// top-level folders a device can map one of (spec: EP-9).
async fn a_book(fixture: &FetchUnderTest, keys: &LibraryKeys, volumes: [&str; 2]) -> Book {
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;

    let pages: Vec<(String, Vec<u8>)> = volumes
        .iter()
        .enumerate()
        .flat_map(|(volume, folder)| {
            (0..PAGES_PER_VOLUME).map(move |page| {
                (
                    format!("{folder}/{page:03}.jpg"),
                    filler(PAGE_LEN, 0x40 + (volume * PAGES_PER_VOLUME + page) as u8),
                )
            })
        })
        .collect();
    for (relative, content) in &pages {
        write(fixture.fs(), fixture.source_folder(), relative, content);
    }
    let frozen = freeze_source(fixture, keys, ONE_PACK, 1).await;
    assert_eq!(
        frozen.packs.len(),
        1,
        "both volumes land in one Pack (spec: PK-5)"
    );

    let container_id = entry_at(fixture.source(), &pages[0].0).await.container_id;
    let summary = fixture
        .source()
        .containers_under(None)
        .await
        .expect("asking the source catalog for its Containers must succeed")
        .into_iter()
        .find(|container| container.id == container_id)
        .expect("the Pack is current");
    let object = container_handle(fixture.store(), container_id).await;
    let body_start = body_start(fixture.store(), &object).await;
    let book = Book {
        pages,
        object,
        container_id,
        body_start,
        object_len: summary.ciphertext_len.get(),
    };

    // The layout every case below is written against, checked by the rule
    // rather than assumed: four parcels, and pages 2, 5 and 7 each running
    // across a boundary while the rest lie inside one (spec: PK-19).
    assert_eq!(parcel_count(book.body_start, book.object_len), 4);
    let mut layout = Vec::new();
    for index in 0..book.pages.len() {
        layout.push(book.parcels_of(fixture, index).await);
    }
    assert_eq!(
        layout,
        [0..1, 0..1, 0..2, 1..2, 1..2, 1..3, 2..3, 2..4],
        "the pages fall across the parcels the way the cases expect",
    );
    book
}

/// Maps the target device's folder at the Library root.
async fn map_the_root(fixture: &FetchUnderTest) {
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;
}

/// The ranges a counting store was asked for of one object past its front.
fn chunk_reads(counting: &CountingStore<'_>, book: &Book) -> Vec<Range<u64>> {
    counting
        .ranges_of(&book.object)
        .into_iter()
        .flatten()
        .filter(|range| range.start >= book.body_start)
        .collect()
}

/// The parcels the target device holds of one Container, by index.
async fn held(fixture: &FetchUnderTest, container_id: ContainerId) -> Vec<u64> {
    held_rows(fixture, container_id)
        .await
        .into_iter()
        .map(|parcel| parcel.index)
        .collect()
}

/// The rows behind [`held`].
async fn held_rows(fixture: &FetchUnderTest, container_id: ContainerId) -> Vec<HeldParcel> {
    fixture
        .target()
        .held_parcels()
        .await
        .expect("asking the target catalog for its held parcels must succeed")
        .into_iter()
        .filter(|parcel| parcel.container_id == container_id)
        .collect()
}

/// Fetches one page into the target device, through `store`, expecting it
/// placed.
async fn fetch_page(
    store: &dyn ObjectStore,
    fixture: &FetchUnderTest,
    keys: &LibraryKeys,
    path: &str,
    run: i64,
) -> EntryFetchOutcome {
    fetch_entry(entry_request(store, fixture, keys, path, run))
        .await
        .unwrap_or_else(|error| panic!("fetching {path} must succeed: {error}"))
}
