use std::fs;

use super::{mappings, set_mapping};
use crate::error::{Error, NameDefect};
use crate::testing::{create_s3, state_dir};

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
    assert_eq!(listed.len(), 2, "remapping replaces rather than adds");
    assert_eq!(listed[1].local_root, moved.canonicalize().unwrap());
}

// A prefix that is not one top-level component names a subtree no mapping can
// stand for, and a root that has never existed is a typo rather than the
// unavailable root EP-12 is about.
#[tokio::test]
async fn a_prefix_with_a_separator_and_a_root_that_is_not_there_are_refused() {
    create_s3("refusals").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");

    let nested = set_mapping("refusals", Some("albums/2026"), folders.path()).await;
    assert!(
        matches!(
            &nested,
            Err(Error::MalformedMappingPrefix {
                defect: NameDefect::Separator,
                ..
            })
        ),
        "expected a nested prefix to be refused, got {nested:?}"
    );

    let missing = set_mapping("refusals", None, &folders.path().join("never-existed")).await;
    assert!(
        matches!(&missing, Err(Error::NoSuchLocalRoot { .. })),
        "expected a root that is not there to be refused, got {missing:?}"
    );

    assert!(mappings("refusals")
        .await
        .expect("the mappings must read")
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
