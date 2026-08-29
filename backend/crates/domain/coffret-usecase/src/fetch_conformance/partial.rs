use coffret_model::{EntryPath, Mtime};

use crate::device_state::LocalEntryState;
use crate::fetch::{fetch_entry, EntryFetch, FetchError};
use crate::fetch_conformance::counting_store::CountingStore;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    body_start, container_handle, entry_at, entry_request, exists, filler, freeze_source, keys,
    map, observed, plant, read, scratch_left, write, Planted, OLDER,
};
use crate::fetch_conformance::mangling_store::ManglingStore;

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
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    let files: Vec<(String, Vec<u8>)> = (0..FILES)
        .map(|index| {
            (
                format!("books/atlas/{index:03}.jpg"),
                filler(FILE_LEN, 0x40 + index as u8),
            )
        })
        .collect();
    for (relative, content) in &files {
        write(fixture.source_folder(), relative, content).await;
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
    let fetched = fetch_entry(entry_request(&counting, fixture.target(), &keys, wanted, 2))
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
        asked < summary.ciphertext_len,
        "reading one Entry asked for {asked} of the Pack's {} bytes",
        summary.ciphertext_len,
    );

    // And it is still a fetch: the file on disk is the file that left.
    let placed = fixture.target_folder().join(wanted);
    assert_eq!(&read(&placed).await, content);
    let (size, mtime) = observed(&placed).await;
    assert_eq!(size, location.entry.size);
    assert_eq!(
        mtime, location.entry.mtime,
        "the placed file carries the Entry's own modification time (spec: FM-9, EP-11)",
    );

    let local = fixture
        .target()
        .local_entry_at(&EntryPath::nfc(wanted.clone()))
        .await
        .expect("asking the target catalog for a local row must succeed")
        .expect("this device placed the file, so it has a row for it");
    assert_eq!(local.state, LocalEntryState::Present);
    assert_eq!(
        scratch_left(fixture.target_folder()).await,
        0,
        "a placed file leaves no temporary one behind (spec: EP-11)",
    );

    // The rest of the Pack is as unfetched as it was: PK-16's range read is a
    // step inside fetching a Container and not a fetch of one.
    for (relative, _) in files.iter().filter(|(relative, _)| relative != wanted) {
        assert!(
            !exists(&fixture.target_folder().join(relative)).await,
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
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    write(fixture.source_folder(), "a.jpg", &filler(2_000, 0x11)).await;
    write(fixture.source_folder(), "b.jpg", &filler(3_000, 0x22)).await;
    freeze_source(fixture, &keys, ONE_PACK, 1).await;

    let location = entry_at(fixture.source(), "b.jpg").await;
    let object = container_handle(fixture.store(), location.container_id).await;
    let chunks = body_start(fixture.store(), &object).await;
    let mangling = ManglingStore::beyond(fixture.store(), object, chunks);

    let result = fetch_entry(entry_request(
        &mangling,
        fixture.target(),
        &keys,
        "b.jpg",
        2,
    ))
    .await;

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
        !exists(&fixture.target_folder().join("b.jpg")).await,
        "nothing unverified reaches a target path (spec: EP-11)",
    );
    assert_eq!(
        scratch_left(fixture.target_folder()).await,
        0,
        "and the temporary file the run made is gone",
    );
    assert!(
        fixture
            .target()
            .local_entry_at(&EntryPath::nfc("b.jpg"))
            .await
            .expect("asking the target catalog for a local row must succeed")
            .is_none(),
        "a run that placed nothing claims nothing (spec: EP-10)",
    );

    // And a later run, against a store that answers honestly, gets the file.
    let fetched = fetch_entry(entry_request(
        fixture.store(),
        fixture.target(),
        &keys,
        "b.jpg",
        3,
    ))
    .await
    .expect("a run against an honest store must succeed");
    assert_eq!(fetched, EntryFetch::Placed);
    assert_eq!(
        read(&fixture.target_folder().join("b.jpg")).await,
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
    map(fixture.target(), None, fixture.target_folder()).await;

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
        },
    )
    .await;

    let result = fetch_entry(entry_request(
        fixture.store(),
        fixture.target(),
        &keys,
        "a.jpg",
        2,
    ))
    .await;

    let Err(FetchError::ContentMismatch { container_id, path }) = result else {
        panic!("expected content the catalog does not name to be refused, got {result:?}");
    };
    assert_eq!(container_id, planted);
    assert_eq!(path, EntryPath::nfc("a.jpg"));

    assert!(
        !exists(&fixture.target_folder().join("a.jpg")).await,
        "an authentic Container is still not the content the catalog names (spec: EP-11)",
    );
    assert_eq!(scratch_left(fixture.target_folder()).await, 0);
}
