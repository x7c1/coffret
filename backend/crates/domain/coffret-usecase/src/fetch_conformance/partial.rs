use coffret_model::Mtime;

use crate::device_state::LocalEntryState;
use crate::entry_paths::entry_path;
use crate::fetch::{fetch_entry, EntryFetch, FetchError};
use crate::fetch_conformance::counting_store::CountingStore;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    body_start, container_handle, entry_at, entry_request, exists, filler, freeze_source, keys,
    map, observed, plant, read, scratch_left, write, Planted, OLDER,
};
use crate::fetch_conformance::mangling_store::ManglingStore;
use crate::fetch_conformance::shortening_store::ShorteningStore;

/// One file of the Pack the range-read case builds.
///
/// The chunk size is a per-Container format parameter and the encoder writes
/// 1 MiB (spec: FM-6), so a Pack a range read can save anything on has to be
/// several chunks long — a Pack that fits in one chunk is a Pack a reader has to
/// read whole whatever it asks for. Eight of these make a Pack of some four
/// chunks, which is the smallest arrangement in which "read one Entry" and "read
/// the Pack" are different amounts of work.
const FILE_LEN: usize = 400 * 1024;

/// How many of them go into the one Pack.
const FILES: usize = 8;

/// A size target roomy enough that all of them land in one Pack (spec: PK-5).
const ONE_PACK: u64 = 16 * 1024 * 1024;

/// One Entry is read out of a Pack without the Pack being read.
///
/// This is what PK-16's range-read clause is for. The fetch unit is still the
/// whole Container — the rest of this Pack is exactly as unfetched afterwards as
/// it was before — but a reader that wants one page of an unfetched book does
/// not wait for the gigabyte around it. The Container says where everything in
/// it is before any of it arrives (spec: FM-2, FM-5, FM-9), so the run reads the
/// object's front and then the chunks covering that one Entry.
///
/// The claim is about which bytes were asked for, which nothing the call returns
/// can carry, so the reads are counted: every read of the Pack carried a range,
/// and they add up to less than the Pack. And it is still a fetch — the file on
/// disk is the file that left the other device, stamped with the Entry's own
/// modification time and recorded as this device's own materialization
/// (spec: EP-10, EP-11).
pub async fn one_entry_is_read_out_of_a_pack_without_reading_the_pack(fixture: &FetchUnderTest) {
    let keys = keys();
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;

    let files: Vec<(String, Vec<u8>)> = (0..FILES)
        .map(|index| {
            (
                format!("books/atlas/{index:03}.jpg"),
                filler(FILE_LEN, 0x40 + index as u8),
            )
        })
        .collect();
    for (relative, content) in &files {
        write(fixture.fs(), fixture.source_folder(), relative, content);
    }
    let frozen = freeze_source(fixture, &keys, ONE_PACK, 1).await;
    assert_eq!(
        frozen.packs.len(),
        1,
        "the target is roomy enough that one Pack holds them all (spec: PK-5)",
    );

    // Somewhere in the middle of the Pack, so the read the case is about starts
    // well past the object's front and ends well before its end.
    let (wanted, content) = &files[FILES / 2];
    let location = entry_at(fixture.source(), wanted).await;
    let summary = fixture
        .source()
        .containers_under(None)
        .await
        .expect("asking the source catalog for its Containers must succeed")
        .into_iter()
        .find(|container| container.id == location.container_id)
        .expect("the Pack the Entry lives in is current");
    let object = container_handle(fixture.store(), location.container_id).await;

    let counting = CountingStore::around(fixture.store());
    let fetched = fetch_entry(entry_request(&counting, fixture, &keys, wanted, 2))
        .await
        .unwrap_or_else(|error| panic!("a partial fetch must succeed: {error}"));
    assert_eq!(fetched, EntryFetch::Placed);

    // What was asked of the Pack, which is the whole point of the case.
    let ranges = counting.ranges_of(&object);
    assert!(!ranges.is_empty(), "the Pack was read at all");
    assert!(
        ranges.iter().all(Option::is_some),
        "every read of the Pack carried a range: {ranges:?}",
    );
    let asked: u64 = ranges
        .iter()
        .flatten()
        .map(|range| range.end - range.start)
        .sum();
    assert!(
        asked < summary.ciphertext_len.get(),
        "reading one Entry asked for {asked} of the Pack's {} bytes",
        summary.ciphertext_len.get(),
    );

    // And it is still a fetch: the file on disk is the file that left.
    let placed = fixture.target_folder().join(wanted);
    assert_eq!(&read(fixture.fs(), &placed), content);
    let (size, mtime) = observed(fixture.fs(), &placed);
    assert_eq!(size, location.entry.extent.size());
    assert_eq!(
        mtime, location.entry.mtime,
        "the placed file carries the Entry's own modification time (spec: FM-9, EP-11)",
    );

    let local = fixture
        .target()
        .local_entry_at(&entry_path(wanted.clone()))
        .await
        .expect("asking the target catalog for a local row must succeed")
        .expect("this device placed the file, so it has a row for it");
    assert_eq!(local.state, LocalEntryState::Present);
    assert_eq!(
        scratch_left(fixture.fs(), fixture.target_folder()),
        0,
        "a placed file leaves no scratch behind (spec: EP-11)",
    );

    // The rest of the Pack is as unfetched as it was: PK-16's range read is a
    // step inside fetching a Container and not a fetch of one.
    for (relative, _) in files.iter().filter(|(relative, _)| relative != wanted) {
        assert!(
            !exists(fixture.fs(), &fixture.target_folder().join(relative)),
            "{relative} was not placed by a fetch of another Entry",
        );
    }
}

/// A damaged chunk inside the range a partial fetch asked for is refused, and
/// nothing becomes visible.
///
/// A range read cannot check the object's own hash — that hash is a claim about
/// bytes it deliberately did not ask for. What holds over a range is per-chunk
/// authentication: each chunk carries its own tag, over its own position in this
/// object and this Container's header as associated data (spec: FM-5, FM-7,
/// FM-8). So damage inside the requested range is caught by the format layer
/// before a byte of it reaches a caller's buffer, which is what makes it safe to
/// ask for part of an object at all.
///
/// The damage happens in transit, which is the only place it can be tested from,
/// and only from the chunk sequence onwards: an object whose header or meta
/// section came back damaged is refused before a chunk is ever aimed at, which
/// is a different refusal.
pub async fn a_mangled_chunk_in_a_partial_fetch_is_refused(fixture: &FetchUnderTest) {
    let keys = keys();
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;

    write(
        fixture.fs(),
        fixture.source_folder(),
        "a.jpg",
        &filler(2_000, 0x11),
    );
    write(
        fixture.fs(),
        fixture.source_folder(),
        "b.jpg",
        &filler(3_000, 0x22),
    );
    freeze_source(fixture, &keys, ONE_PACK, 1).await;

    let location = entry_at(fixture.source(), "b.jpg").await;
    let object = container_handle(fixture.store(), location.container_id).await;
    let chunks = body_start(fixture.store(), &object).await;
    let mangling = ManglingStore::beyond(fixture.store(), object, chunks);

    let result = fetch_entry(entry_request(&mangling, fixture, &keys, "b.jpg", 2)).await;

    let Err(FetchError::Format(error)) = result else {
        panic!("expected a damaged chunk to be refused, got {result:?}");
    };
    // Which refusal it is, is the point of the case: the tag over the chunk is
    // the gate that holds over a range at all, so the verdict has to be that tag
    // failing and not some check the fetch made up for itself (spec: FM-5,
    // FM-8).
    assert!(
        matches!(error, coffret_format::Error::AuthenticationFailed),
        "expected the damaged chunk's own tag to refuse it, got {error:?}",
    );

    assert!(
        !exists(fixture.fs(), &fixture.target_folder().join("b.jpg")),
        "nothing unverified reaches a target path (spec: EP-11)",
    );
    assert_eq!(
        scratch_left(fixture.fs(), fixture.target_folder()),
        0,
        "and the scratch the run made is gone",
    );
    assert!(
        fixture
            .target()
            .local_entry_at(&entry_path("b.jpg"))
            .await
            .expect("asking the target catalog for a local row must succeed")
            .is_none(),
        "a run that placed nothing claims nothing (spec: EP-10)",
    );

    // And a later run, against a store that answers honestly, gets the file.
    let fetched = fetch_entry(entry_request(fixture.store(), fixture, &keys, "b.jpg", 3))
        .await
        .expect("a run against an honest store must succeed");
    assert_eq!(fetched, EntryFetch::Placed);
    assert_eq!(
        read(fixture.fs(), &fixture.target_folder().join("b.jpg")),
        filler(3_000, 0x22)
    );
}

/// An Entry whose plaintext is not the content the catalog names is refused
/// before the rename.
///
/// The object authenticates: every chunk verifies against the Container's own
/// header and entry table, so the bytes really are a coffret object sealed under
/// the key the committed Keyring maps this Container to. What they do not agree
/// with is the entry table the *Journal record* carried, which is what the Index
/// answers from (spec: CP-11). That comparison is the last gate before a fetched
/// file becomes visible, and a range read leans on it harder than a whole-object
/// fetch does — it is the only end-to-end check either has once the object's own
/// hash is out of reach (spec: EP-11).
pub async fn a_partial_fetch_of_content_the_catalog_does_not_name_is_refused(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;

    let planted = plant(
        fixture.store(),
        fixture.source(),
        &keys,
        Planted {
            path: "a.jpg",
            content: b"the content the record's entry table describes",
            mtime: Mtime::from_unix_seconds(OLDER),
            real: true,
            actual_content: Some(b"the content the object really holds"),
            meta_len: None,
            short_by: None,
        },
    )
    .await;

    let result = fetch_entry(entry_request(fixture.store(), fixture, &keys, "a.jpg", 2)).await;

    let Err(FetchError::ContentMismatch { container_id, path }) = result else {
        panic!("expected content the catalog does not name to be refused, got {result:?}");
    };
    assert_eq!(container_id, planted);
    assert_eq!(path, entry_path("a.jpg"));

    assert!(
        !exists(fixture.fs(), &fixture.target_folder().join("a.jpg")),
        "an authentic Container is still not the content the catalog names (spec: EP-11)",
    );
    assert_eq!(scratch_left(fixture.fs(), fixture.target_folder()), 0);
}

/// A ranged read of the chunks answered short is Storage's doing, and is asked
/// again.
///
/// The run a partial fetch asks for is placed by the Container's own header and
/// meta section, and held against the object's recorded length before it is
/// asked for (spec: FM-2, FM-5, FM-15), so every byte of it is one Storage
/// holds. An answer that keeps to the length it declares and declares less than
/// the run is a provider or a proxy cutting the range short — the same family as
/// a stream that ends before its own declaration — and the retry policy is
/// where that goes. What this case rules out is the chunk decoder reporting it
/// as a run that ended short, which is a verdict about the Library and would
/// never be asked again.
pub async fn a_short_ranged_read_of_the_chunks_is_asked_again(fixture: &FetchUnderTest) {
    let keys = keys();
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;

    write(
        fixture.fs(),
        fixture.source_folder(),
        "a.jpg",
        &filler(2_000, 0x33),
    );
    freeze_source(fixture, &keys, ONE_PACK, 1).await;

    let location = entry_at(fixture.source(), "a.jpg").await;
    let object = container_handle(fixture.store(), location.container_id).await;
    let chunks = body_start(fixture.store(), &object).await;
    let counting = CountingStore::around(fixture.store());
    let shortening = ShorteningStore::beyond(&counting, object.clone(), chunks);

    let fetched = fetch_entry(entry_request(&shortening, fixture, &keys, "a.jpg", 2))
        .await
        .unwrap_or_else(|error| panic!("a short answer asked again must come through: {error}"));
    assert_eq!(fetched, EntryFetch::Placed);
    assert!(
        shortening.has_shortened(),
        "the case gave the short answer it is about"
    );

    // The chunk run was asked for twice: once answered short, and once whole.
    let runs: Vec<_> = counting
        .ranges_of(&object)
        .into_iter()
        .flatten()
        .filter(|range| range.start >= chunks)
        .collect();
    assert_eq!(
        runs.len(),
        2,
        "the short answer was asked again, and only once: {runs:?}",
    );
    assert_eq!(
        runs[0], runs[1],
        "the second request asked for the same run"
    );
    assert_eq!(
        read(fixture.fs(), &fixture.target_folder().join("a.jpg")),
        filler(2_000, 0x33),
    );
}

/// A Container whose header places chunks past the end of its own object is
/// refused, and nothing is asked again.
///
/// The object is exactly the one the Library committed — the record measures
/// and hashes what is stored — and it is shorter than its authenticated header
/// and meta section say (spec: FM-2, FM-15). That is a verdict about the Library
/// and not a transfer that went wrong: no second request makes the object any
/// longer. So the run is held against the recorded length before it is asked
/// for, and the case checks what was asked of Storage: the object's front, and
/// no read of the chunks at all.
pub async fn a_header_placing_chunks_past_its_object_is_not_asked_again(fixture: &FetchUnderTest) {
    let keys = keys();
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;

    let planted = plant(
        fixture.store(),
        fixture.source(),
        &keys,
        Planted {
            path: "a.jpg",
            content: b"the content the record's entry table describes",
            mtime: Mtime::from_unix_seconds(OLDER),
            real: true,
            actual_content: None,
            meta_len: None,
            // One byte off the last chunk: the front is whole, and the run the
            // one Entry needs ends a byte past the object.
            short_by: Some(1),
        },
    )
    .await;
    let object = container_handle(fixture.store(), planted).await;

    let counting = CountingStore::around(fixture.store());
    let result = fetch_entry(entry_request(&counting, fixture, &keys, "a.jpg", 2)).await;

    let Err(FetchError::Format(error)) = result else {
        panic!("expected a header lying about its lengths to be refused, got {result:?}");
    };
    assert!(
        matches!(error, coffret_format::Error::Truncated),
        "expected the object to be refused as shorter than its header says, got {error:?}",
    );
    let ranges = counting.ranges_of(&object);
    assert_eq!(
        ranges.len(),
        2,
        "the header and the meta section were read, and nothing after them: {ranges:?}",
    );

    assert!(
        !exists(fixture.fs(), &fixture.target_folder().join("a.jpg")),
        "nothing unverified reaches a target path (spec: EP-11)",
    );
    assert_eq!(scratch_left(fixture.fs(), fixture.target_folder()), 0);
}
