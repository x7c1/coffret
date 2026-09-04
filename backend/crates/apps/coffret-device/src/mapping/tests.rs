use std::fs;

use rusqlite::Connection;

use super::{mappings, set_mapping};
use crate::error::Error;
use crate::mapping_listing::MappingListing;
use crate::testing::{create_s3, state_dir};
use crate::{LibraryDir, ModelError, PathDefect};

// EP-9: a device maps the Library root and top-level components, at most one
// mapping each, and mapping a component again moves it.
#[tokio::test]
async fn mappings_are_listed_root_first_and_remapping_moves_a_prefix() {
    create_s3("mapped").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");
    let root = folders.path().join("library");
    let albums = folders.path().join("albums");
    let moved = folders.path().join("albums-moved");
    for path in [&root, &albums, &moved] {
        fs::create_dir(path).expect("the folder must be creatable");
    }

    set_mapping("mapped", Some("albums"), &albums)
        .await
        .expect("a top-level component must be mappable");
    set_mapping("mapped", None, &root)
        .await
        .expect("the Library root must be mappable");

    let listed = mappings("mapped").await.expect("the mappings must read");
    let listed = listed.mappings();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].prefix, None);
    assert_eq!(listed[0].local_root, root.canonicalize().unwrap());
    assert_eq!(
        listed[1].prefix.as_ref().map(|p| p.as_str()),
        Some("albums")
    );
    assert_eq!(listed[1].local_root, albums.canonicalize().unwrap());

    set_mapping("mapped", Some("albums"), &moved)
        .await
        .expect("a mapped component must be movable");
    let listed = mappings("mapped").await.expect("the mappings must read");
    let listed = listed.mappings();
    assert_eq!(listed.len(), 2, "remapping replaces rather than adds");
    assert_eq!(listed[1].local_root, moved.canonicalize().unwrap());
}

// EP-9: a mapping is keyed by exactly one top-level component, so a prefix
// naming a subtree stands for nothing a mapping can hold. What a component may
// be spelled with is EP-2's and not this device's: `/` is an Entry Path's only
// logical separator, so a backslash is an ordinary character in a folder name
// and a folder called `a\b` is mapped like any other.
#[tokio::test]
async fn a_mapping_prefix_with_more_than_one_component_is_refused() {
    create_s3("prefixes").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");
    let albums = folders.path().join("albums");
    let odd = folders.path().join("odd");
    for path in [&albums, &odd] {
        fs::create_dir(path).expect("the folder must be creatable");
    }

    let nested = set_mapping("prefixes", Some("albums/2026"), &albums).await;
    assert!(
        matches!(
            &nested,
            Err(Error::MalformedMappingPrefix { cause: None, .. })
        ),
        "expected a prefix of more than one component to be refused, got {nested:?}"
    );

    set_mapping("prefixes", Some("albums"), &albums)
        .await
        .expect("one top-level component must be mappable");
    set_mapping("prefixes", Some("a\\b"), &odd)
        .await
        .expect("a backslash is a character a top-level component may carry");
}

// A prefix that is no Entry Path at all is the model's refusal rather than this
// crate's, and it arrives carrying the part of the shape it failed
// (spec: EP-2).
#[tokio::test]
async fn a_mapping_prefix_that_is_no_entry_path_is_refused_in_the_models_words() {
    create_s3("shapes").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");

    let trailing = set_mapping("shapes", Some("albums/"), folders.path()).await;
    assert!(
        matches!(
            &trailing,
            Err(Error::MalformedMappingPrefix {
                cause: Some(ModelError::MalformedEntryPath {
                    defect: PathDefect::TrailingSeparator,
                    ..
                }),
                ..
            })
        ),
        "expected a trailing separator to be refused as the model reads it, got {trailing:?}"
    );
}

// A root that has never existed is a typo rather than the unavailable root
// EP-12 is about, so it is refused instead of recorded.
#[tokio::test]
async fn a_local_root_that_has_never_existed_is_refused() {
    create_s3("refusals").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");

    let missing = set_mapping("refusals", None, &folders.path().join("never-existed")).await;
    assert!(
        matches!(&missing, Err(Error::NoSuchLocalRoot { .. })),
        "expected a root that is not there to be refused, got {missing:?}"
    );

    assert!(mappings("refusals")
        .await
        .expect("the mappings must read")
        .mappings()
        .is_empty());
}

// A mapping is recorded in the catalog, so asking for one of a Library that is
// not here must not be the thing that creates its catalog.
#[tokio::test]
async fn mapping_a_library_that_is_not_here_creates_no_catalog() {
    state_dir();
    let folders = tempfile::tempdir().expect("a temporary directory must be available");

    let result = set_mapping("never-created", None, folders.path()).await;
    assert!(
        matches!(&result, Err(Error::NoSuchLibrary { name, .. }) if name == "never-created"),
        "expected a Library that is not here to be refused, got {result:?}"
    );
    assert!(!state_dir().join("libraries").join("never-created").exists());
}

// The mappings are the one piece of device state a refused Index file still
// gives up: its two columns stay readable in every layout, so a layout this
// build cannot open falls back to reading them straight from the file instead
// of losing the listing along with the catalog.
#[tokio::test]
async fn mappings_are_still_listed_when_the_index_is_refused() {
    create_s3("old-layout").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");
    let root = folders.path().join("library");
    let albums = folders.path().join("albums");
    for path in [&root, &albums] {
        fs::create_dir(path).expect("the folder must be creatable");
    }

    set_mapping("old-layout", None, &root)
        .await
        .expect("the Library root must be mappable");
    set_mapping("old-layout", Some("albums"), &albums)
        .await
        .expect("a top-level component must be mappable");

    // Below this build's device-local floor, written out rather than read from
    // the gateway, which keeps its own schema stamps to itself — the sibling
    // suite in `coffret-sqlite-index` does the same.
    const BELOW_DEVICE_SCHEMA_VERSION: i64 = 3;
    let index_file = LibraryDir::resolve("old-layout")
        .expect("the name is a valid Library name")
        .index_file();
    Connection::open(&index_file)
        .expect("the Index file must open")
        .pragma_update(None, "user_version", BELOW_DEVICE_SCHEMA_VERSION)
        .expect("stamping a version must succeed");

    let listing = mappings("old-layout")
        .await
        .expect("a refused file still yields its mappings");
    assert!(
        matches!(
            &listing,
            MappingListing::FromRefusedFile {
                refusal: coffret_usecase::IndexError::UnsupportedSchema { .. },
                ..
            }
        ),
        "expected the variant carrying the refusal, got {listing:?}"
    );
    let read = listing.mappings();
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].prefix, None);
    assert_eq!(read[1].prefix.as_ref().map(|p| p.as_str()), Some("albums"));
}
