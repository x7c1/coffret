//! Reading a planted catalog as folders.
//!
//! Nothing here reaches Storage or a file, because nothing browsing does: the
//! answers come out of the current Entries and this device's own materialization
//! records, and both are catalog rows. So the cases plant those rows through the
//! same [`Index`] the flows write them through — a Journal record for what the
//! Library holds, and a present observation for what this device has — and read
//! the listing back.

use std::sync::Arc;

use coffret_model::{
    CiphertextLenClaim, ContainerAddition, ContainerId, ContainerKind, ContainerSummary,
    ContentHash, DepositedAt, EntryExtent, EntryMetadata, EntryPath, Generation, JournalRecord,
    KeyringCommitment, LibraryId, MasterKey, MasterKeyEpoch, Mtime,
};
use coffret_usecase::device_state::{DeviceTime, LocalObservation, Mapping};
use coffret_usecase::{EntryRef, InMemoryIndex, InMemoryStore, Index, LibraryKeys};

use crate::browse::EntryState;
use crate::open_library::OpenLibrary;
use crate::testing::{entry_path, local_fs};

/// A Library whose catalog holds the Entries at `paths` and nothing else.
///
/// One Container per call, its kind given, so a case can put a Pack and a
/// one-file Container side by side and ask what a row says about each.
async fn library(planted: &[(u8, ContainerKind, &[&str])]) -> OpenLibrary {
    let index = InMemoryIndex::new();
    for (seed, kind, paths) in planted {
        index
            .apply(record(*seed, *kind, paths))
            .await
            .expect("a record the catalog has never seen replays");
    }
    OpenLibrary {
        store: Arc::new(InMemoryStore::new(64)),
        index: Arc::new(index),
        local_fs: local_fs(),
        keys: LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        ),
        spool: std::env::temp_dir(),
        parcel_dir: std::env::temp_dir().join("parcels"),
        parcel_len: coffret_format::PARCEL_LEN,
        library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
        name: None,
        epoch: MasterKeyEpoch::FIRST,
        provider: "s3",
        grant: None,
        births: Default::default(),
        fetch_memory: Default::default(),
        live: Default::default(),
    }
}

/// The generation `number` names, or a panic naming the literal that names
/// none: every number the format carries is bounded (spec: FM-19), a case's
/// own seed included.
fn generation_of(number: u64) -> Generation {
    Generation::new(number)
        .unwrap_or_else(|error| panic!("a case holds a literal generation: {error}"))
}

/// When the Entry at `at` of the Container `seed` names was deposited: a
/// second of its own for each, and none of them its `mtime`.
fn deposited_at(seed: u8, at: usize) -> DepositedAt {
    DepositedAt::from_unix_seconds(1_760_000_000 + i64::from(seed) * 100 + at as i64)
}

/// A record adding one Container of `kind` holding an Entry at each path.
///
/// The generation is the seed, so a case planting two Containers replays two
/// records in the order it named them.
fn record(seed: u8, kind: ContainerKind, paths: &[&str]) -> JournalRecord {
    let generation = generation_of(u64::from(seed));
    let addition = ContainerAddition::new(
        ContainerSummary {
            id: ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]),
            kind,
            ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
            ciphertext_len: CiphertextLenClaim::new(1_024)
                .expect("a case's own literal is a length the format admits"),
            object_ref: None,
        },
        paths
            .iter()
            .enumerate()
            .map(|(at, path)| EntryMetadata {
                path: entry_path(*path),
                captured_path: entry_path(*path),
                extent: EntryExtent::new(at as u64 * 100, 100)
                    .expect("a case's own Entries lie inside the address space"),
                mtime: Mtime::from_unix_seconds(1_700_000_000 + at as i64),
                btime: None,
                hash: ContentHash::from_bytes([seed.wrapping_add(at as u8); 32]),
                derived_from: None,
                mime: None,
            })
            .collect(),
        (0..paths.len()).map(|at| deposited_at(seed, at)).collect(),
    )
    .expect("a case's own entry table tiles its Container's stream");

    JournalRecord::new(
        generation,
        seed.checked_sub(1)
            .map(|prev| generation_of(u64::from(prev))),
        MasterKeyEpoch::FIRST,
        KeyringCommitment::new(generation, 3, "beef")
            .expect("a lowercase hex digest and a non-zero count are a valid commitment"),
        None,
        None,
        vec![addition],
        vec![],
    )
    .expect("a case's own record succeeds the head one generation back")
}

/// Records that this device has the file for one Entry (spec: EP-10).
async fn hold(library: &OpenLibrary, path: &str) {
    let entry = library
        .index
        .entry_at(&entry_path(path))
        .await
        .expect("the catalog answers")
        .expect("a case holds a file of an Entry the Library holds");
    library
        .index
        .mark_present(LocalObservation {
            path: entry_path(path),
            size: 100,
            mtime: Mtime::from_unix_seconds(1_700_000_000),
            at: DeviceTime::from_unix_seconds(1_700_000_100),
            entry: EntryRef::of(&entry),
            hash: None,
        })
        .await
        .expect("a present observation is recorded");
}

/// Records that a folder on this device stands for one part of the Library
/// (spec: EP-9); `None` is the Library root.
///
/// Where the folder is does not matter to a listing — a mapping asserts nothing
/// about what is on disk — so every one of these is rooted at a name under the
/// temporary directory that no case ever creates or reads.
async fn map(library: &OpenLibrary, prefix: Option<&str>) {
    library
        .index
        .set_mapping(Mapping::new(
            prefix.map(entry_path),
            std::env::temp_dir().join(prefix.unwrap_or("library-root")),
        ))
        .await
        .expect("a mapping is recorded");
}

/// The names of what a listing holds, folders first.
fn names(listing: &crate::browse::FolderListing) -> (Vec<&str>, Vec<&str>) {
    (
        listing
            .folders
            .iter()
            .map(|folder| folder.name.as_str())
            .collect(),
        listing
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect(),
    )
}

// A folder is what the separators imply and nothing the catalog stores
// (spec: EP-2), so one level down is exactly the next component: the folders
// named by everything with a separator left in it, and the files by everything
// without one.
#[tokio::test]
async fn a_folder_holds_its_own_children_and_not_its_grandchildren() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &[
            "albums/2026/spring.jpg",
            "albums/2026/summer.jpg",
            "albums/cover.png",
            "books/page-001.png",
        ],
    )])
    .await;

    let listing = library
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    assert_eq!(names(&listing), (vec!["2026"], vec!["cover.png"]));
    assert_eq!(
        listing.folders[0].path.as_str(),
        "albums/2026",
        "a child folder is named by its whole path, not only its last component",
    );
}

// EP-3: the order is the byte order of the canonical paths, with no case folding
// and no locale. `Z` is 0x5a and `a` is 0x61, so a locale-aware ordering would
// put these the other way round — which is the point of pinning it.
#[tokio::test]
async fn the_listing_is_in_byte_order() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &[
            "albums/b.jpg",
            "albums/A.jpg",
            "albums/Zurich/one.jpg",
            "albums/aachen/one.jpg",
        ],
    )])
    .await;

    let listing = library
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    assert_eq!(
        names(&listing),
        (vec!["Zurich", "aachen"], vec!["A.jpg", "b.jpg"])
    );
}

// FM-15: a row carries the deposit time the catalog holds for its Entry, beside
// and apart from the file's own modification time.
#[tokio::test]
async fn a_row_carries_its_entrys_deposit_time() {
    let library = library(&[(1, ContainerKind::Pack, &["albums/a.jpg", "albums/b.jpg"])]).await;

    let listing = library
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    let rows: Vec<(&str, DepositedAt, Mtime)> = listing
        .files
        .iter()
        .map(|row| (row.name.as_str(), row.deposited_at, row.mtime))
        .collect();
    assert_eq!(
        rows,
        [
            (
                "a.jpg",
                deposited_at(1, 0),
                Mtime::from_unix_seconds(1_700_000_000)
            ),
            (
                "b.jpg",
                deposited_at(1, 1),
                Mtime::from_unix_seconds(1_700_000_001)
            ),
        ],
    );
}

// EP-10: a mapping asserts nothing about what is on disk, so the only thing that
// makes a row present is this device's own materialization record.
#[tokio::test]
async fn a_row_is_present_only_where_this_device_materialized_it() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &["albums/here.jpg", "albums/elsewhere.jpg"],
    )])
    .await;
    hold(&library, "albums/here.jpg").await;

    let listing = library
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    let states: Vec<(&str, EntryState)> = listing
        .files
        .iter()
        .map(|file| (file.name.as_str(), file.state))
        .collect();
    assert_eq!(
        states,
        vec![
            ("elsewhere.jpg", EntryState::Remote),
            ("here.jpg", EntryState::Present),
        ],
    );
    assert_eq!(
        library
            .state_of(&entry_path("albums/here.jpg"))
            .await
            .expect("the catalog answers"),
        EntryState::Present,
    );
}

/// Whether each child folder of `folder` is present throughout, by name.
async fn throughout(library: &OpenLibrary, folder: Option<&str>) -> Vec<(String, bool)> {
    library
        .list(folder.map(entry_path).as_ref())
        .await
        .expect("the catalog answers")
        .folders
        .into_iter()
        .map(|child| (child.name, child.present_throughout))
        .collect()
}

// EP-10: a folder is here whole only where every Entry at every depth below it
// has a present record — the promise that the whole book, or the whole series,
// reads without Storage. One file anywhere below that this device does not
// have, however deep, takes the promise back.
#[tokio::test]
async fn a_folder_is_present_throughout_only_where_every_file_below_it_is() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &[
            // Every file here, one level down and two.
            "books/all/page-1.png",
            "books/all/vol-2/page-1.png",
            // One file missing two levels down.
            "books/deep/page-1.png",
            "books/deep/vol-2/page-1.png",
            // A file whose fetch failed: it left no present record.
            "books/failed/page-1.png",
            "books/failed/page-2.png",
            // A file that was here and that this device witnessed go.
            "books/gone/page-1.png",
            // No file of its own, only a folder below it, which is here whole.
            "books/nested/vol-1/page-1.png",
        ],
    )])
    .await;
    for path in [
        "books/all/page-1.png",
        "books/all/vol-2/page-1.png",
        "books/deep/page-1.png",
        "books/failed/page-1.png",
        "books/gone/page-1.png",
        "books/nested/vol-1/page-1.png",
    ] {
        hold(&library, path).await;
    }
    library
        .index
        .mark_absent(
            &entry_path("books/gone/page-1.png"),
            DeviceTime::from_unix_seconds(1_700_000_200),
        )
        .await
        .expect("an absent observation is recorded");

    assert_eq!(
        throughout(&library, Some("books")).await,
        vec![
            ("all".to_owned(), true),
            ("deep".to_owned(), false),
            ("failed".to_owned(), false),
            ("gone".to_owned(), false),
            ("nested".to_owned(), true),
        ],
    );
    // The root answers for `books` from the same subtree: not whole, since
    // some of it is not here.
    assert_eq!(
        throughout(&library, None).await,
        vec![("books".to_owned(), false)]
    );
    // And one level further down, each folder answers for its own subtree.
    assert_eq!(
        throughout(&library, Some("books/deep")).await,
        vec![("vol-2".to_owned(), false)],
    );
    assert_eq!(
        throughout(&library, Some("books/all")).await,
        vec![("vol-2".to_owned(), true)],
    );
}

/// How many files stand below each child folder of `folder`, and how many of
/// them are here, by name.
async fn counted(library: &OpenLibrary, folder: Option<&str>) -> Vec<(String, u64, u64)> {
    library
        .list(folder.map(entry_path).as_ref())
        .await
        .expect("the catalog answers")
        .folders
        .into_iter()
        .map(|child| (child.name, child.files_present, child.files_total))
        .collect()
}

// EP-10: a folder counts every Entry at every depth below it, and those this
// device has a present record for — "18 of 196 files" — out of the same scan
// that answers whether all of them are here. A file one level down and a file
// three levels down count alike, and a file that left this device does not
// count as here.
#[tokio::test]
async fn a_folder_counts_the_files_below_it_and_those_here_at_every_depth() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &[
            "series/vol-1/page-1.png",
            "series/vol-1/page-2.png",
            "series/vol-2/page-1.png",
            "series/vol-2/extra/deep/page-1.png",
            "series/cover.png",
            "loose/a.txt",
            "loose/b.txt",
            "top.png",
        ],
    )])
    .await;
    for path in [
        "series/vol-1/page-1.png",
        "series/vol-1/page-2.png",
        "series/vol-2/extra/deep/page-1.png",
        "loose/a.txt",
    ] {
        hold(&library, path).await;
    }
    library
        .index
        .mark_absent(
            &entry_path("loose/a.txt"),
            DeviceTime::from_unix_seconds(1_700_000_200),
        )
        .await
        .expect("an absent observation is recorded");

    // The root counts each top-level folder's whole subtree, and not the file
    // standing beside them.
    assert_eq!(
        counted(&library, None).await,
        vec![("loose".to_owned(), 0, 2), ("series".to_owned(), 3, 5)],
    );
    // One level down, each volume counts its own subtree, the file three
    // levels below `vol-2` included.
    assert_eq!(
        counted(&library, Some("series")).await,
        vec![("vol-1".to_owned(), 2, 2), ("vol-2".to_owned(), 1, 2)],
    );
    // Deeper still, a folder holding only a folder counts what is below that.
    assert_eq!(
        counted(&library, Some("series/vol-2")).await,
        vec![("extra".to_owned(), 1, 1)],
    );
    // The bytes add up over the same subtree, here or not: every case Entry is
    // 100 bytes long.
    let sizes: Vec<(String, u64)> = library
        .list(None)
        .await
        .expect("the catalog answers")
        .folders
        .into_iter()
        .map(|child| (child.name, child.size))
        .collect();
    assert_eq!(
        sizes,
        vec![("loose".to_owned(), 200), ("series".to_owned(), 500)],
    );
    // The yes-or-no agrees with the counts wherever it is asked.
    let whole: Vec<(String, bool)> = counted(&library, Some("series"))
        .await
        .into_iter()
        .map(|(name, here, of)| (name, here == of))
        .collect();
    assert_eq!(throughout(&library, Some("series")).await, whole);
}

// A folder has no time of its own (spec: EP-2), so its Modified is the latest
// Entry time at any depth below it. The planting order gives each Entry the
// next second, and the paths are chosen so the newest one stands deepest and
// is neither the first nor the last the catalog's EP-3 order reaches: taking
// the first or the last time seen would each answer something else.
#[tokio::test]
async fn a_folder_is_modified_when_the_newest_file_at_any_depth_below_it_was() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &["book/a.png", "book/z.png", "book/m/deep/page.png"],
    )])
    .await;

    let times: Vec<(String, Mtime)> = library
        .list(None)
        .await
        .expect("the catalog answers")
        .folders
        .into_iter()
        .map(|child| (child.name, child.mtime))
        .collect();
    assert_eq!(
        times,
        vec![("book".to_owned(), Mtime::from_unix_seconds(1_700_000_002))],
    );
}

// PK-15: the kind is what says whether an Entry can be replaced one file at a
// time, so it travels in the row rather than being asked for afterwards.
#[tokio::test]
async fn a_row_says_which_kind_of_container_holds_it() {
    let library = library(&[
        (1, ContainerKind::Pack, &["albums/packed.jpg"]),
        (2, ContainerKind::OneFile, &["albums/alone.jpg"]),
    ])
    .await;

    let listing = library
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    let kinds: Vec<(&str, ContainerKind)> = listing
        .files
        .iter()
        .map(|file| (file.name.as_str(), file.container))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("alone.jpg", ContainerKind::OneFile),
            ("packed.jpg", ContainerKind::Pack),
        ],
    );
}

// The Library admits a file and a folder at one path — neither is a thing it
// has — and the file is not a child of the folder named after it.
#[tokio::test]
async fn an_entry_standing_at_the_folder_itself_is_not_in_its_listing() {
    let library = library(&[(1, ContainerKind::Pack, &["albums", "albums/spring.jpg"])]).await;

    let listing = library
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    assert_eq!(names(&listing), (Vec::<&str>::new(), vec!["spring.jpg"]));

    // From the root it is an ordinary file beside the folder of the same name.
    let root = library.list(None).await.expect("the catalog answers");
    assert_eq!(names(&root), (vec!["albums"], vec!["albums"]));
}

// Every folder, flat, each named by its whole path — and every ancestor of an
// Entry several components deep, since whatever nests them has only this to
// nest.
#[tokio::test]
async fn every_folder_of_the_library_is_named_by_its_whole_path() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &[
            "albums/2026/08/spring.jpg",
            "books/some-novel/page-001.png",
            "notes.txt",
        ],
    )])
    .await;

    let folders = library.folders().await.expect("the catalog answers");
    let named: Vec<&str> = folders.iter().map(EntryPath::as_str).collect();
    assert_eq!(
        named,
        vec![
            "albums",
            "albums/2026",
            "albums/2026/08",
            "books",
            "books/some-novel"
        ],
        "a file at the root implies no folder, and every ancestor of one is a folder",
    );
}

// EP-2: `/` is the only logical separator, so a sibling whose name merely begins
// with the same letters is not under the folder — `-` is 0x2d and sorts before
// `/`, which is exactly where a comparison against the bare prefix would go
// wrong.
#[tokio::test]
async fn a_sibling_that_starts_with_the_same_letters_is_not_inside() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &["books/page-001.png", "books-annex/page-001.png"],
    )])
    .await;

    let listing = library
        .list(Some(&entry_path("books")))
        .await
        .expect("the catalog answers");
    assert_eq!(names(&listing), (Vec::<&str>::new(), vec!["page-001.png"]));
}

// A catalog that stands at no committed state holds no Entry, so there is
// nothing to browse and that is an answer rather than a failure (spec: CP-1).
#[tokio::test]
async fn a_library_that_holds_nothing_lists_nothing() {
    let library = library(&[]).await;

    assert!(library
        .folders()
        .await
        .expect("the catalog answers")
        .is_empty());
    let listing = library.list(None).await.expect("the catalog answers");
    assert_eq!(names(&listing), (Vec::<&str>::new(), Vec::<&str>::new()));
    assert_eq!(listing.path, None);
    assert_eq!(
        library
            .state_of(&entry_path("albums/nothing.jpg"))
            .await
            .expect("the catalog answers"),
        EntryState::Remote,
    );
}

// A folder is what the separators under it imply, so one the Library has never
// held answers with the same empty lists an empty folder would — and the only
// empty folder there is, is the root. The listing says which of the two it is,
// because nothing in the rows can.
#[tokio::test]
async fn a_listing_says_whether_the_library_has_this_folder_at_all() {
    let planted = library(&[(1, ContainerKind::Pack, &["albums/cover.png"])]).await;

    let albums = planted
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    assert!(albums.held, "something is under it, which is what has it");

    let mistyped = planted
        .list(Some(&entry_path("album")))
        .await
        .expect("the catalog answers");
    assert!(!mistyped.held);
    assert_eq!(names(&mistyped), (Vec::<&str>::new(), Vec::<&str>::new()));

    // And a path under a folder the Library does have, which is the same answer
    // for the same reason: nothing stands under this one to imply it. A folder
    // whose last Entry left arrives here too — what implied it is what went.
    let deeper = planted
        .list(Some(&entry_path("albums/2026")))
        .await
        .expect("the catalog answers");
    assert!(!deeper.held);

    // And the root of a Library holding nothing, whose lists are identical to
    // both of those: it is the Library rather than a path, so it is there.
    let empty = library(&[]).await;
    let root = empty.list(None).await.expect("the catalog answers");
    assert!(root.held);
    assert_eq!(names(&root), (Vec::<&str>::new(), Vec::<&str>::new()));
}

// EP-9: a folder no mapping reaches has nowhere on this device to put a file,
// and the listing says so — before a reader clicks a row and waits out a fetch
// that could only be declined.
#[tokio::test]
async fn a_listing_says_whether_this_device_has_a_folder_for_it() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &["albums/cover.png", "books/page-001.png", "notes.txt"],
    )])
    .await;
    map(&library, Some("albums")).await;

    let albums = library
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    assert!(albums.mapped, "the mapped subtree is on this device");

    let books = library
        .list(Some(&entry_path("books")))
        .await
        .expect("the catalog answers");
    assert!(!books.mapped, "nothing on this device stands for books");

    // The root itself is not reached either: a top-level mapping stands for its
    // own subtree, so `notes.txt` beside it has no folder here to go in.
    let root = library.list(None).await.expect("the catalog answers");
    assert!(!root.mapped);
    let reached: Vec<(&str, bool)> = root
        .folders
        .iter()
        .map(|folder| (folder.name.as_str(), folder.mapped))
        .collect();
    assert_eq!(
        reached,
        vec![("albums", true), ("books", false)],
        "the children of the root are where two siblings can differ",
    );
}

// EP-2, EP-9: a top-level folder this device maps with no Entry under it is no
// folder of the Library, and the root's listing names it apart from the
// folders, so a page can offer it as a place to drop into. One that holds an
// Entry is an ordinary folder and is not named twice.
#[tokio::test]
async fn the_root_names_the_mapped_top_level_folders_nothing_is_under() {
    let library = library(&[(1, ContainerKind::Pack, &["albums/cover.png"])]).await;
    map(&library, Some("albums")).await;
    map(&library, Some("comics")).await;

    let root = library.list(None).await.expect("the catalog answers");
    assert_eq!(
        root.folders
            .iter()
            .map(|folder| folder.name.as_str())
            .collect::<Vec<_>>(),
        ["albums"],
    );
    assert_eq!(root.empty_mapped_prefixes, [entry_path("comics")]);

    // Only the root's listing names them: a mapping is made at the top level.
    let albums = library
        .list(Some(&entry_path("albums")))
        .await
        .expect("the catalog answers");
    assert!(albums.empty_mapped_prefixes.is_empty());
}

// EP-9: a mapping at the Library root represents everything the top-level ones
// do not, so with one present nothing is out of reach.
#[tokio::test]
async fn a_root_mapping_reaches_every_folder() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &["albums/2026/spring.jpg", "books/page-001.png"],
    )])
    .await;
    map(&library, None).await;

    let root = library.list(None).await.expect("the catalog answers");
    assert!(root.mapped);
    assert!(root.folders.iter().all(|folder| folder.mapped));

    for folder in ["albums", "albums/2026", "books"] {
        let listing = library
            .list(Some(&entry_path(folder)))
            .await
            .expect("the catalog answers");
        assert!(listing.mapped, "{folder} is on this device");
    }
}

// A device that has mapped nothing reaches nothing, which is the state a device
// is in between joining a Library and being told where to keep it.
#[tokio::test]
async fn a_device_that_maps_nothing_reaches_no_folder() {
    let library = library(&[(1, ContainerKind::Pack, &["albums/cover.png"])]).await;

    let root = library.list(None).await.expect("the catalog answers");
    assert!(!root.mapped);
    assert_eq!(
        root.folders.iter().map(|f| f.mapped).collect::<Vec<_>>(),
        [false]
    );
}

/// The files either side of a row, as text.
async fn beside(
    library: &OpenLibrary,
    path: &str,
    folder: bool,
) -> (Option<String>, Option<String>) {
    let found = library
        .files_beside(&entry_path(path), folder)
        .await
        .expect("the catalog answers");
    (
        found.previous.map(|one| one.as_str().to_owned()),
        found.next.map(|one| one.as_str().to_owned()),
    )
}

fn some(path: &str) -> Option<String> {
    Some(path.to_owned())
}

// EP-3: reading order is the tree's with every folder open — a folder's
// folders before its files, each in byte order — so the file after the last
// of one folder is the first of the next, at whatever depth, and a folder's
// own files come after everything in its folders. Folders that nothing has
// listed are read as readily as any: the answer is the catalog's.
#[tokio::test]
async fn the_files_beside_a_row_are_in_reading_order_across_folders() {
    let library = library(&[
        (
            1,
            ContainerKind::Pack,
            &["series/vol-1/001.jpg", "series/vol-1/002.jpg"],
        ),
        (
            2,
            ContainerKind::Pack,
            &["series/vol-2/001.jpg", "series/vol-2/extra/a.jpg"],
        ),
        (
            3,
            ContainerKind::OneFile,
            &["series/notes.txt", "series/a.txt", "series.txt"],
        ),
    ])
    .await;

    // Off the end of one folder into the next.
    assert_eq!(
        beside(&library, "series/vol-1/002.jpg", false).await,
        (
            some("series/vol-1/001.jpg"),
            some("series/vol-2/extra/a.jpg")
        ),
        "a folder's folders come before its files, whatever their names",
    );
    assert_eq!(
        beside(&library, "series/vol-2/extra/a.jpg", false).await,
        (some("series/vol-1/002.jpg"), some("series/vol-2/001.jpg")),
    );
    // Out of the last folder, onto the files of the folder above it.
    assert_eq!(
        beside(&library, "series/vol-2/001.jpg", false).await,
        (some("series/vol-2/extra/a.jpg"), some("series/a.txt")),
    );
    assert_eq!(
        beside(&library, "series/notes.txt", false).await,
        (some("series/a.txt"), some("series.txt")),
    );
    // The two ends of the Library.
    assert_eq!(
        beside(&library, "series/vol-1/001.jpg", false).await,
        (None, some("series/vol-1/002.jpg")),
    );
    assert_eq!(
        beside(&library, "series.txt", false).await,
        (some("series/notes.txt"), None),
    );
}

// A folder's place is just before everything under it, so a reader on a
// folder reads on into its first file and steps back onto the file before it.
#[tokio::test]
async fn the_files_beside_a_folder_are_its_first_and_the_one_before_it() {
    let library = library(&[(
        1,
        ContainerKind::Pack,
        &["a/1.jpg", "b/c/1.jpg", "b/c/2.jpg", "b/1.jpg", "d.jpg"],
    )])
    .await;

    assert_eq!(
        beside(&library, "b", true).await,
        (some("a/1.jpg"), some("b/c/1.jpg"))
    );
    assert_eq!(
        beside(&library, "b/c", true).await,
        (some("a/1.jpg"), some("b/c/1.jpg"))
    );
    assert_eq!(beside(&library, "a", true).await, (None, some("a/1.jpg")));
    // A path the Library holds nothing at names a place all the same.
    assert_eq!(
        beside(&library, "b/0", true).await,
        (some("a/1.jpg"), some("b/c/1.jpg"))
    );
    assert_eq!(
        beside(&library, "b/z.jpg", false).await,
        (some("b/1.jpg"), some("d.jpg"))
    );
}
