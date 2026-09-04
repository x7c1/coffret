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
    ContentHash, EntryExtent, EntryMetadata, EntryPath, Generation, JournalRecord,
    KeyringCommitment, LibraryId, MasterKey, MasterKeyEpoch, Mtime,
};
use coffret_usecase::device_state::{DeviceTime, LocalObservation, Mapping};
use coffret_usecase::{InMemoryIndex, InMemoryStore, Index, LibraryKeys};

use crate::browse::EntryState;
use crate::open_library::OpenLibrary;
use crate::testing::entry_path;

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
        keys: LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        ),
        spool: std::env::temp_dir(),
        library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
        epoch: MasterKeyEpoch::FIRST,
        provider: "s3",
    }
}

/// A record adding one Container of `kind` holding an Entry at each path.
///
/// The generation is the seed, so a case planting two Containers replays two
/// records in the order it named them.
fn record(seed: u8, kind: ContainerKind, paths: &[&str]) -> JournalRecord {
    let generation = Generation::new(u64::from(seed));
    let addition = ContainerAddition::new(
        ContainerSummary {
            id: ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]),
            kind,
            ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
            ciphertext_len: CiphertextLenClaim::new(1_024),
            object_ref: None,
        },
        paths
            .iter()
            .enumerate()
            .map(|(at, path)| EntryMetadata {
                path: entry_path(*path),
                extent: EntryExtent::new(at as u64 * 100, 100)
                    .expect("a case's own Entries lie inside the address space"),
                mtime: Mtime::from_unix_seconds(1_700_000_000 + at as i64),
                btime: None,
                hash: ContentHash::from_bytes([seed.wrapping_add(at as u8); 32]),
                derived_from: None,
                mime: None,
            })
            .collect(),
    )
    .expect("a case's own entry table tiles its Container's stream");

    JournalRecord::new(
        generation,
        seed.checked_sub(1)
            .map(|prev| Generation::new(u64::from(prev))),
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
    library
        .index
        .mark_present(LocalObservation {
            path: entry_path(path),
            size: 100,
            mtime: Mtime::from_unix_seconds(1_700_000_000),
            at: DeviceTime::from_unix_seconds(1_700_000_100),
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
        .set_mapping(Mapping {
            prefix: prefix.map(entry_path),
            local_root: std::env::temp_dir().join(prefix.unwrap_or("library-root")),
            root_identity: None,
        })
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
