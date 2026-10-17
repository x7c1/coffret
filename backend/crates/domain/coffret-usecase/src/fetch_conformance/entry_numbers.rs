//! Reading an Entry out of its Container by its entry number (spec: FM-9,
//! FM-15, EP-11).
//!
//! A rename changes the path the catalog holds an Entry at and nothing in the
//! Container holding it: the entry table still records the path the Entry was
//! captured under. So a reader finds the Entry in that table by its entry
//! number, and places the bytes at the path the catalog holds now. The cases
//! rename inside a Pack and a one-file Container, swap two Entries' names in
//! one record — where a lookup by path would find the other Entry — and rename
//! one of several Entries of no bytes, which share one offset in the stream
//! and are told apart by their numbers alone. A whole fetch and a parcel read
//! are each held to all of it, since they find Entries in two places. A file
//! this device placed before the rename waits where it stands while the fetch
//! places the Entry at its new path (spec: EP-10).

use coffret_format::Purpose;
use coffret_model::{ContainerId, EntryLocation, EntryRename, Mtime};

use crate::commit_conformance::commit_renames_in;
use crate::entry_paths::entry_path;
use crate::fetch::{fetch_entry, fetch_folders, EntryFetch, FetchError, LibraryKeys, Relocation};
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    entry_at, entry_request, exists, freeze_source, keys, map, observed, plant, purpose_key, read,
    request, scratch_left, sync_source, write, Planted, OLDER,
};
use crate::sync_conformance::fixtures::touch;

/// A Pack target no case here comes near, so every frozen file lands in one
/// Pack (spec: PK-5).
const ONE_PACK: u64 = 16 * 1024 * 1024;

const VOL1: &[u8] = b"the first volume";
const VOL2: &[u8] = b"the second volume, a little longer";
const VOL3: &[u8] = b"the third volume";
const COVER: &[u8] = b"a cover kept in a Container of its own";

/// The rename of the Entry at `location` to `path`.
fn rename(location: &EntryLocation, path: &str) -> EntryRename {
    EntryRename::new(
        location.container_id,
        location.entry_number,
        entry_path(path),
    )
    .expect("an ordinary path is a rename's target")
}

/// Commits `renames` the way another device's rename lands, behind the
/// source device's catalog: the target device catches it up on its next run.
async fn commit_renames(fixture: &FetchUnderTest, renames: Vec<EntryRename>) {
    commit_renames_in(
        fixture.store(),
        fixture.source(),
        &purpose_key(Purpose::ControlJournal),
        renames,
    )
    .await;
}

/// Where the source device's catalog holds the four Entries of
/// [`a_library`] before any rename.
struct Frozen {
    vol1: EntryLocation,
    vol2: EntryLocation,
    vol3: EntryLocation,
    cover: EntryLocation,
}

/// A Library of one Pack holding `books/vol1.txt`, `books/vol2.txt` and
/// `books/vol3.txt`, and one one-file Container holding `albums/cover.jpg`,
/// with the target device mapped to receive it.
async fn a_library(fixture: &FetchUnderTest, keys: &LibraryKeys) -> Frozen {
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    for (relative, content) in [
        ("books/vol1.txt", VOL1),
        ("books/vol2.txt", VOL2),
        ("books/vol3.txt", VOL3),
    ] {
        write(fixture.fs(), fixture.source_folder(), relative, content);
    }
    let frozen = freeze_source(fixture, keys, ONE_PACK, 1).await;
    assert_eq!(frozen.packs.len(), 1, "the volumes land in one Pack");
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/cover.jpg",
        COVER,
    );
    sync_source(fixture, keys, 2).await;

    let frozen = Frozen {
        vol1: entry_at(fixture.source(), "books/vol1.txt").await,
        vol2: entry_at(fixture.source(), "books/vol2.txt").await,
        vol3: entry_at(fixture.source(), "books/vol3.txt").await,
        cover: entry_at(fixture.source(), "albums/cover.jpg").await,
    };
    assert_ne!(frozen.cover.container_id, frozen.vol1.container_id);
    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;
    frozen
}

/// Renames [`a_library`]'s Entries: in the Pack `books/vol1.txt` and
/// `books/vol2.txt` swap their names and `books/vol3.txt` is given
/// `books/later/vol3.txt`, and the one-file Container's `albums/cover.jpg` is
/// given `albums/front.jpg`.
async fn rename_all(fixture: &FetchUnderTest, frozen: &Frozen) {
    commit_renames(
        fixture,
        vec![
            rename(&frozen.vol1, "books/vol2.txt"),
            rename(&frozen.vol2, "books/vol1.txt"),
            rename(&frozen.vol3, "books/later/vol3.txt"),
            rename(&frozen.cover, "albums/front.jpg"),
        ],
    )
    .await;
}

/// [`a_library`], renamed by [`rename_all`] before the target device fetches
/// any of it. Answers the Pack's Container ID.
async fn a_renamed_library(fixture: &FetchUnderTest, keys: &LibraryKeys) -> ContainerId {
    let frozen = a_library(fixture, keys).await;
    rename_all(fixture, &frozen).await;
    frozen.vol1.container_id
}

/// What the target device's folder holds at `relative`.
fn placed(fixture: &FetchUnderTest, relative: &str) -> Vec<u8> {
    read(fixture.fs(), &fixture.target_folder().join(relative))
}

/// Whether anything stands at `relative` in the target device's folder.
fn stands(fixture: &FetchUnderTest, relative: &str) -> bool {
    exists(fixture.fs(), &fixture.target_folder().join(relative))
}

/// A whole fetch places every renamed Entry's own bytes at its current path:
/// the swapped pair each at the other's former name, the renamed Pack Entry
/// and the renamed one-file Entry at their new names, and nothing at a path
/// the Library no longer holds (spec: FM-9, FM-15, EP-11).
pub async fn a_renamed_entry_is_fetched_whole_at_its_current_path(fixture: &FetchUnderTest) {
    let keys = keys();
    a_renamed_library(fixture, &keys).await;

    let outcome = fetch_folders(request(fixture.store(), fixture, &keys, 3))
        .await
        .unwrap_or_else(|error| panic!("a fetch of renamed Entries must succeed: {error}"));

    assert_eq!(
        outcome.fetched,
        [
            "albums/front.jpg",
            "books/later/vol3.txt",
            "books/vol1.txt",
            "books/vol2.txt",
        ]
        .map(entry_path),
    );
    assert_eq!(placed(fixture, "books/vol1.txt"), VOL2);
    assert_eq!(placed(fixture, "books/vol2.txt"), VOL1);
    assert_eq!(placed(fixture, "books/later/vol3.txt"), VOL3);
    assert_eq!(placed(fixture, "albums/front.jpg"), COVER);
    assert!(!stands(fixture, "books/vol3.txt"));
    assert!(!stands(fixture, "albums/cover.jpg"));
    assert_eq!(scratch_left(fixture.fs(), fixture.target_folder()), 0);
}

/// A parcel read places the renamed Entry asked for, and every Entry its
/// parcels cover, at their current paths with their own bytes — and the
/// parcels are let go once those Entries are on the device at those paths
/// (spec: FM-15, PK-16, PK-21). A one-file Container's renamed Entry is read
/// the same way.
pub async fn a_renamed_entry_is_read_by_its_parcels_at_its_current_path(fixture: &FetchUnderTest) {
    let keys = keys();
    let pack = a_renamed_library(fixture, &keys).await;

    let page = fetch_entry(entry_request(
        fixture.store(),
        fixture,
        &keys,
        "books/vol1.txt",
        3,
    ))
    .await
    .unwrap_or_else(|error| panic!("reading a renamed Entry must succeed: {error}"));
    assert_eq!(page.fetch, EntryFetch::Placed);
    assert_eq!(placed(fixture, "books/vol1.txt"), VOL2);
    assert_eq!(
        page.alongside,
        ["books/vol2.txt", "books/later/vol3.txt"].map(entry_path),
        "the rest of the Pack, in stream order, at the paths the catalog holds now",
    );
    assert_eq!(placed(fixture, "books/vol2.txt"), VOL1);
    assert_eq!(placed(fixture, "books/later/vol3.txt"), VOL3);
    assert!(!stands(fixture, "books/vol3.txt"));
    assert!(
        fixture
            .target()
            .held_parcels()
            .await
            .expect("asking the target catalog for its held parcels must succeed")
            .iter()
            .all(|parcel| parcel.container_id != pack),
        "every Entry the parcels cover is placed, so none is kept",
    );

    let cover = fetch_entry(entry_request(
        fixture.store(),
        fixture,
        &keys,
        "albums/front.jpg",
        4,
    ))
    .await
    .unwrap_or_else(|error| panic!("reading a renamed one-file Entry must succeed: {error}"));
    assert_eq!(cover.fetch, EntryFetch::Placed);
    assert!(cover.alongside.is_empty());
    assert_eq!(placed(fixture, "albums/front.jpg"), COVER);
    assert!(!stands(fixture, "albums/cover.jpg"));
}

/// A file this device placed before its Entry was renamed is relocated to the
/// Entry's current path before the fetch selects anything, and the fetch then
/// finds every place already holding this device's own copy: the two files of
/// the swapped Entries exchange their names, the others are relocated to theirs, and
/// nothing is fetched, overwritten or left behind (spec: FM-15, EP-10, EP-11).
pub async fn a_fetch_relocates_the_files_of_renamed_entries_before_it_places(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    let frozen = a_library(fixture, &keys).await;
    fetch_folders(request(fixture.store(), fixture, &keys, 3))
        .await
        .unwrap_or_else(|error| panic!("the first fetch must succeed: {error}"));
    rename_all(fixture, &frozen).await;

    let outcome = fetch_folders(request(fixture.store(), fixture, &keys, 4))
        .await
        .unwrap_or_else(|error| panic!("a fetch after the renames must succeed: {error}"));

    let mut relocated: Vec<(String, String)> = outcome
        .relocations
        .iter()
        .map(|one| match one {
            Relocation::Relocated { from, to } => {
                (from.as_str().to_owned(), to.as_str().to_owned())
            }
            other => panic!("every waiting file is relocated, got {other:?}"),
        })
        .collect();
    relocated.sort();
    assert_eq!(
        relocated,
        [
            ("albums/cover.jpg", "albums/front.jpg"),
            ("books/vol1.txt", "books/vol2.txt"),
            ("books/vol2.txt", "books/vol1.txt"),
            ("books/vol3.txt", "books/later/vol3.txt"),
        ]
        .map(|(from, to)| (from.to_owned(), to.to_owned())),
    );
    assert!(outcome.fetched.is_empty(), "{:?}", outcome.fetched);
    assert!(outcome.surfaced.is_empty(), "{:?}", outcome.surfaced);
    assert_eq!(
        outcome.skipped, 4,
        "every place holds this device's own copy"
    );
    for (relative, content) in [
        ("books/vol2.txt", VOL1),
        ("books/vol1.txt", VOL2),
        ("books/later/vol3.txt", VOL3),
        ("albums/front.jpg", COVER),
    ] {
        assert_eq!(placed(fixture, relative), content, "{relative}");
    }
    assert!(!stands(fixture, "books/vol3.txt"));
    assert!(!stands(fixture, "albums/cover.jpg"));
    assert!(fixture.fs().discarded().is_empty(), "nothing was discarded");
    assert_eq!(scratch_left(fixture.fs(), fixture.target_folder()), 0);
}

/// The modification time each Entry of no bytes is stamped with, which is
/// what tells their files apart once placed (spec: FM-9, EP-11).
const EMPTY: [(&str, i64); 3] = [
    ("notes/m.txt", OLDER + 1),
    ("notes/n.txt", OLDER + 2),
    ("notes/z.txt", OLDER + 3),
];

/// A Pack holding one Entry of some bytes and three of none, all three at
/// one offset in the stream, and `notes/n.txt` renamed to
/// `notes/renamed.txt` after it was committed.
async fn empty_entries_one_renamed(fixture: &FetchUnderTest, keys: &LibraryKeys) {
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "notes/a.txt",
        b"before",
    );
    for (relative, mtime) in EMPTY {
        let file = write(fixture.fs(), fixture.source_folder(), relative, b"");
        touch(fixture.fs(), &file, mtime);
    }
    let frozen = freeze_source(fixture, keys, ONE_PACK, 1).await;
    assert_eq!(frozen.packs.len(), 1, "the notes land in one Pack");

    let n = entry_at(fixture.source(), "notes/n.txt").await;
    let m = entry_at(fixture.source(), "notes/m.txt").await;
    assert_eq!(
        n.extent(),
        m.extent(),
        "Entries of no bytes share an offset"
    );
    commit_renames(fixture, vec![rename(&n, "notes/renamed.txt")]).await;

    map(
        fixture.target(),
        fixture.fs(),
        None,
        fixture.target_folder(),
    )
    .await;
}

/// Each Entry of no bytes is placed at its current path with its own
/// modification time, the renamed one included.
fn assert_each_empty_entry_is_its_own(fixture: &FetchUnderTest) {
    for (captured, mtime) in EMPTY {
        let relative = match captured {
            "notes/n.txt" => "notes/renamed.txt",
            unrenamed => unrenamed,
        };
        let file = fixture.target_folder().join(relative);
        assert_eq!(read(fixture.fs(), &file), b"", "{relative}");
        assert_eq!(
            observed(fixture.fs(), &file).1,
            Mtime::from_unix_seconds(mtime),
            "{relative} is its own Entry, found by its number",
        );
    }
    assert_eq!(placed(fixture, "notes/a.txt"), b"before");
    assert!(!stands(fixture, "notes/n.txt"));
}

/// Several Entries of no bytes in one Pack, one of them renamed, are each
/// fetched whole as their own Entry (spec: FM-9, FM-15, EP-11).
pub async fn entries_of_no_bytes_are_each_fetched_whole_after_a_rename(fixture: &FetchUnderTest) {
    let keys = keys();
    empty_entries_one_renamed(fixture, &keys).await;

    fetch_folders(request(fixture.store(), fixture, &keys, 2))
        .await
        .unwrap_or_else(|error| panic!("a fetch of the notes must succeed: {error}"));

    assert_each_empty_entry_is_its_own(fixture);
}

/// Several Entries of no bytes in one Pack, one of them renamed, are each
/// placed as their own Entry by a parcel read of the renamed one
/// (spec: FM-9, FM-15, PK-16).
pub async fn entries_of_no_bytes_are_each_read_by_their_parcel_after_a_rename(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    empty_entries_one_renamed(fixture, &keys).await;

    let page = fetch_entry(entry_request(
        fixture.store(),
        fixture,
        &keys,
        "notes/renamed.txt",
        2,
    ))
    .await
    .unwrap_or_else(|error| panic!("reading a renamed Entry of no bytes must succeed: {error}"));

    assert_eq!(page.fetch, EntryFetch::Placed);
    assert_eq!(
        page.alongside,
        ["notes/a.txt", "notes/m.txt", "notes/z.txt"].map(entry_path),
    );
    assert_each_empty_entry_is_its_own(fixture);
}

/// An entry number the Container's own table does not reach is refused as an
/// Entry missing from it, naming the number, by a whole fetch and a parcel
/// read alike — never looked for under its path instead (spec: FM-9, CP-11).
pub async fn an_entry_number_its_container_does_not_reach_is_refused(fixture: &FetchUnderTest) {
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
            content: b"the one Entry the object holds",
            mtime: Mtime::from_unix_seconds(OLDER),
            real: true,
            actual_content: None,
            meta_len: None,
            short_by: None,
            misrecorded: false,
            unheld: Some("b.jpg"),
        },
    )
    .await;

    let read = fetch_entry(entry_request(fixture.store(), fixture, &keys, "b.jpg", 2)).await;
    let whole = fetch_folders(request(fixture.store(), fixture, &keys, 3)).await;

    for (how, result) in [
        ("a parcel read", read.map(|_| ())),
        ("a whole fetch", whole.map(|_| ())),
    ] {
        let Err(FetchError::EntryMissing {
            container_id,
            entry_number,
            path,
        }) = result
        else {
            panic!("{how} of an entry number past the table must be refused, got {result:?}");
        };
        assert_eq!(container_id, planted, "{how}");
        assert_eq!(entry_number.get(), 1, "{how}");
        assert_eq!(path, entry_path("b.jpg"), "{how}");
    }
    assert!(!stands(fixture, "b.jpg"));
    assert_eq!(scratch_left(fixture.fs(), fixture.target_folder()), 0);
}
