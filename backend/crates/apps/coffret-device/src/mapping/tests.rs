use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker;
use rusqlite::Connection;

use super::{mappings, set_mapping};
use crate::error::{Error, Result};
use crate::mapping_listing::MappingListing;
use crate::marker_record::MarkerRecord;
use crate::marker_request::MarkerRequest;
use crate::recorded_mapping::RecordedMapping;
use crate::testing::{create_s3, state_dir};
use crate::{LibraryDir, ModelError, PathDefect};

/// Recording a mapping with no wish about the root's identity, which is what
/// every case here means unless it says otherwise.
async fn map(library: &str, prefix: Option<&str>, local_root: &Path) -> Result<RecordedMapping> {
    set_mapping(library, prefix, local_root, MarkerRequest::AdoptWhatIsThere).await
}

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

    map("mapped", Some("albums"), &albums)
        .await
        .expect("a top-level component must be mappable");
    map("mapped", None, &root)
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

    map("mapped", Some("albums"), &moved)
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

    let nested = map("prefixes", Some("albums/2026"), &albums).await;
    assert!(
        matches!(
            &nested,
            Err(Error::MalformedMappingPrefix { cause: None, .. })
        ),
        "expected a prefix of more than one component to be refused, got {nested:?}"
    );

    map("prefixes", Some("albums"), &albums)
        .await
        .expect("one top-level component must be mappable");
    map("prefixes", Some("a\\b"), &odd)
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

    let trailing = map("shapes", Some("albums/"), folders.path()).await;
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

    let missing = map("refusals", None, &folders.path().join("never-existed")).await;
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

    let result = map("never-created", None, folders.path()).await;
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

    map("old-layout", None, &root)
        .await
        .expect("the Library root must be mappable");
    map("old-layout", Some("albums"), &albums)
        .await
        .expect("a top-level component must be mappable");

    // The layout before this one, written out rather than read from the
    // gateway, which keeps its own schema stamps to itself — the sibling suite
    // in `coffret-sqlite-index` does the same. It is the interesting number
    // now: the change that gave a mapping the identity it expects of its root
    // (spec: EP-13) moved the device-local floor up to the current layout, so
    // the very newest file this build did not write is already refused whole,
    // and the fallback below is what stands between that refusal and the owner
    // losing the record of where their Library lives.
    const PREVIOUS_SCHEMA_VERSION: i64 = 5;
    let index_file = LibraryDir::resolve("old-layout")
        .expect("the name is a valid device-local Library name")
        .index_file();
    Connection::open(&index_file)
        .expect("the Index file must open")
        .pragma_update(None, "user_version", PREVIOUS_SCHEMA_VERSION)
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

/// The marker file's path inside a mapped root (spec: EP-13).
fn marker_in(root: &Path) -> PathBuf {
    root.join(root_marker::MANAGEMENT_AREA)
        .join(root_marker::MARKER_FILE)
}

/// The identity a root's marker names, read straight off the disk rather than
/// through the catalog.
fn identity_in(root: &Path) -> RootMarkerId {
    let content = fs::read(marker_in(root)).expect("the marker must be readable");
    root_marker::parse(&content).expect("the marker must name an identity")
}

/// What one mapping of a Library expects its root's marker to carry.
async fn expected_in(library: &str, prefix: Option<&str>) -> Option<RootMarkerId> {
    let listing = mappings(library).await.expect("the mappings must read");
    listing
        .mappings()
        .iter()
        .find(|mapping| mapping.prefix.as_ref().map(|p| p.as_str()) == prefix)
        .expect("the mapping must be recorded")
        .expected_root_id
}

/// A folder with a management area holding exactly `content` at the marker's
/// name.
fn a_root_whose_marker_holds(folders: &Path, name: &str, content: &[u8]) -> PathBuf {
    let root = folders.join(name);
    fs::create_dir(&root).expect("the folder must be creatable");
    fs::create_dir(root.join(root_marker::MANAGEMENT_AREA))
        .expect("the management area must be creatable");
    fs::write(marker_in(&root), content).expect("the marker must be writable");
    root
}

// EP-13: recording a mapping gives the root an identity of its own — a marker
// file inside the root's management area, and the same identifier kept as what
// the mapping expects to find there. Nothing else in coffret writes it, so if
// this call does not, no root ever carries one.
#[tokio::test]
async fn recording_a_mapping_writes_a_marker_into_the_root() {
    create_s3("marked").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");
    let root = folders.path().join("library");
    fs::create_dir(&root).expect("the folder must be creatable");

    let recorded = map("marked", None, &root)
        .await
        .expect("a root with no marker must be mappable");

    assert_eq!(recorded.marker, MarkerRecord::Written);
    let written = fs::read(marker_in(&root)).expect("the marker must be there");
    assert_eq!(
        written,
        root_marker::spell(&identity_in(&root)),
        "the marker holds the identity spelled the one way it is spelled"
    );
    assert_eq!(
        expected_in("marked", None).await,
        Some(identity_in(&root)),
        "and the mapping expects exactly what was written into the root"
    );
}

// EP-13: a root that already carries a valid marker keeps it. Several mappings,
// or several devices, sharing one root share one identity, and none of them
// destroys another's — which is why recording adopts rather than rewrites.
#[tokio::test]
async fn recording_a_root_that_already_holds_a_valid_marker_keeps_its_id() {
    create_s3("shared-root").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");
    let root = folders.path().join("library");
    fs::create_dir(&root).expect("the folder must be creatable");

    let first = map("shared-root", None, &root)
        .await
        .expect("a root with no marker must be mappable");
    let written = fs::read(marker_in(&root)).expect("the marker must be there");

    let second = map("shared-root", Some("albums"), &root)
        .await
        .expect("a root that already carries a marker must be mappable");

    assert_eq!(first.marker, MarkerRecord::Written);
    assert_eq!(second.marker, MarkerRecord::Adopted);
    assert_eq!(
        fs::read(marker_in(&root)).expect("the marker must still be there"),
        written,
        "adopting writes nothing: the file is byte for byte the one that was there"
    );

    let identity = identity_in(&root);
    assert_eq!(expected_in("shared-root", None).await, Some(identity));
    assert_eq!(
        expected_in("shared-root", Some("albums")).await,
        Some(identity),
        "two mappings of one root expect one identity"
    );
}

// EP-13: content that names no identity is refused rather than repaired. What
// stands there was written by something, and replacing it would take an
// identity away from whichever device that was.
#[tokio::test]
async fn a_root_whose_marker_is_malformed_is_refused_and_nothing_is_written() {
    create_s3("malformed").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");

    for (name, content) in [
        ("gibberish", b"not an identity\n".to_vec()),
        // Uppercase is a hex digit nowhere in coffret, so a marker spelled
        // with one names no identity rather than a second spelling of one.
        ("uppercase", b"00112233445566AA\n".to_vec()),
        // Past the cap a marker is read to, which is settled on the length
        // before any of it is made sense of.
        ("enormous", vec![b'0'; root_marker::MAX_LEN + 1]),
        // A file with nothing in it names no identity like any other content
        // that is not one. It is the shape a marker whose writing was cut short
        // would have, which is why `write_marker` takes its own file away again
        // rather than leave a root in a state no later run repairs.
        ("empty", Vec::new()),
    ] {
        let root = a_root_whose_marker_holds(folders.path(), name, &content);

        let refused = map("malformed", None, &root).await;
        assert!(
            matches!(&refused, Err(Error::MarkerMalformed { .. })),
            "expected the marker in {name} to be refused, got {refused:?}"
        );
        assert_eq!(
            fs::read(marker_in(&root)).expect("the marker must still be there"),
            content,
            "a refusal writes nothing, so what was in {name} is still in it"
        );
    }

    assert!(
        mappings("malformed")
            .await
            .expect("the mappings must read")
            .mappings()
            .is_empty(),
        "and no mapping was recorded against a root whose identity could not be settled"
    );
}

// EP-13: a management area with no marker in it is what an interrupted
// registration leaves. Finishing it would give the root an identity the run
// that made the folder never agreed to, so it is an error instead.
#[tokio::test]
async fn a_root_holding_only_the_management_area_is_refused() {
    create_s3("half-registered").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");
    let root = folders.path().join("library");
    fs::create_dir(&root).expect("the folder must be creatable");
    fs::create_dir(root.join(root_marker::MANAGEMENT_AREA))
        .expect("the management area must be creatable");

    let refused = map("half-registered", None, &root).await;
    assert!(
        matches!(&refused, Err(Error::ManagementAreaIncomplete { .. })),
        "expected a management area with no marker to be refused, got {refused:?}"
    );
    assert!(
        !marker_in(&root).exists(),
        "and the marker the refusal named was not written after all"
    );
    assert!(mappings("half-registered")
        .await
        .expect("the mappings must read")
        .mappings()
        .is_empty());
}

// EP-8, EP-13: below the root, coffret descends without following links. A
// symbolic link at either name is refused rather than followed — the folder the
// person configured is the one the marker is about, and a second name for
// something else is not it.
//
// What a link at each name arrives as follows the flags its open is made with:
// the marker's passes no `O_DIRECTORY`, so it is `ELOOP` on both platforms,
// while the area's passes it and a link there is `ELOOP` on Linux and `ENOTDIR`
// on macOS. Registration reads each of them as the placement side reads it —
// which is what keeps a refusal on one side from being a local I/O failure on
// the other. A platform whose kernel reports something neither side reads is
// what the gateway's own contract test and its compile-time platform gate are
// for.
#[tokio::test]
async fn a_root_whose_marker_is_a_symbolic_link_is_refused() {
    create_s3("linked").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");

    // Somewhere else entirely, holding a marker that would parse perfectly well
    // if anything followed the link to it.
    let elsewhere = a_root_whose_marker_holds(folders.path(), "elsewhere", b"0011223344556677\n");

    let linked_marker = folders.path().join("linked-marker");
    fs::create_dir(&linked_marker).expect("the folder must be creatable");
    fs::create_dir(linked_marker.join(root_marker::MANAGEMENT_AREA))
        .expect("the management area must be creatable");
    symlink(marker_in(&elsewhere), marker_in(&linked_marker))
        .expect("a symbolic link must be creatable");

    let refused = map("linked", None, &linked_marker).await;
    assert!(
        matches!(&refused, Err(Error::MarkerNotARegularFile { .. })),
        "expected a marker that is a symbolic link to be refused, got {refused:?}"
    );

    // And the same at the management area's own name, which is one refusal
    // because what a person does about either is the same.
    let linked_area = folders.path().join("linked-area");
    fs::create_dir(&linked_area).expect("the folder must be creatable");
    symlink(
        elsewhere.join(root_marker::MANAGEMENT_AREA),
        linked_area.join(root_marker::MANAGEMENT_AREA),
    )
    .expect("a symbolic link must be creatable");

    let refused = map("linked", None, &linked_area).await;
    assert!(
        matches!(&refused, Err(Error::ManagementAreaNotADirectory { .. })),
        "expected a management area that is a symbolic link to be refused, got {refused:?}"
    );

    assert!(
        fs::symlink_metadata(marker_in(&linked_marker))
            .expect("the link must still be there")
            .is_symlink(),
        "nothing was written through the link"
    );
    assert_eq!(
        fs::read(marker_in(&elsewhere)).expect("the marker must still be there"),
        b"0011223344556677\n",
        "and nothing was written at the other end of it"
    );
}

// EP-13: each of the two names has a kind it must be, and something else
// standing at either is refused. The link above is one way to be the wrong
// kind; this is the plain one, which needs no link and no `O_NOFOLLOW` to
// arrive — an ordinary file called `.coffret`, or an ordinary folder called
// `root` — and which a person can leave behind by hand.
#[tokio::test]
async fn a_root_whose_management_area_or_marker_is_the_wrong_kind_is_refused() {
    create_s3("wrong-kind").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");

    let file_at_the_area = folders.path().join("file-at-the-area");
    fs::create_dir(&file_at_the_area).expect("the folder must be creatable");
    fs::write(
        file_at_the_area.join(root_marker::MANAGEMENT_AREA),
        b"not a folder\n",
    )
    .expect("the file must be writable");

    let refused = map("wrong-kind", None, &file_at_the_area).await;
    assert!(
        matches!(&refused, Err(Error::ManagementAreaNotADirectory { .. })),
        "expected a file standing at the management area's name to be refused, got {refused:?}"
    );

    // A folder at the marker's name opens where a symbolic link does not, so
    // what it is is asked of the handle rather than of the open.
    let folder_at_the_marker = folders.path().join("folder-at-the-marker");
    fs::create_dir_all(marker_in(&folder_at_the_marker)).expect("the folders must be creatable");

    let refused = map("wrong-kind", Some("albums"), &folder_at_the_marker).await;
    assert!(
        matches!(&refused, Err(Error::MarkerNotARegularFile { .. })),
        "expected a folder standing at the marker's name to be refused, got {refused:?}"
    );

    assert!(
        mappings("wrong-kind")
            .await
            .expect("the mappings must read")
            .mappings()
            .is_empty(),
        "and neither root was recorded, since neither could be given an identity"
    );
}

// EP-13: a person whose two roots ended up sharing an identity — one copied
// from the other — asks for a new one outright. The flag replaces an identity
// and repairs nothing: a root with no marker gets the absent case, and a root
// whose marker is malformed is still an error.
#[tokio::test]
async fn resetting_the_marker_issues_a_new_identity() {
    create_s3("reset").await;
    let folders = tempfile::tempdir().expect("a temporary directory must be available");
    let root = folders.path().join("library");
    fs::create_dir(&root).expect("the folder must be creatable");

    map("reset", None, &root)
        .await
        .expect("a root with no marker must be mappable");
    let first = identity_in(&root);

    let recorded = set_mapping("reset", None, &root, MarkerRequest::IssueANewIdentity)
        .await
        .expect("a root carrying a valid marker must take a new identity");

    assert_eq!(recorded.marker, MarkerRecord::Reset);
    let second = identity_in(&root);
    assert_ne!(
        first, second,
        "the root carries an identity it did not before"
    );
    assert_eq!(
        expected_in("reset", None).await,
        Some(second),
        "and the mapping expects the new one rather than the identity it replaced"
    );
    assert!(
        fs::read_dir(root.join(root_marker::MANAGEMENT_AREA))
            .expect("the management area must be readable")
            .filter_map(|entry| entry.ok())
            .all(|entry| entry.file_name() == root_marker::MARKER_FILE),
        "the file was published by a rename, and the name it was written under is gone"
    );

    // Where there is no identity to replace, the flag asks for what the absent
    // case does anyway.
    let fresh = folders.path().join("fresh");
    fs::create_dir(&fresh).expect("the folder must be creatable");
    let recorded = set_mapping(
        "reset",
        Some("albums"),
        &fresh,
        MarkerRequest::IssueANewIdentity,
    )
    .await
    .expect("a root with no marker must be mappable with or without the flag");
    assert_eq!(recorded.marker, MarkerRecord::Written);

    // Where the identity cannot be read at all, the flag changes nothing: it
    // replaces an identity, it does not repair a broken management area.
    let broken = a_root_whose_marker_holds(folders.path(), "broken", b"not an identity\n");
    let refused = set_mapping(
        "reset",
        Some("books"),
        &broken,
        MarkerRequest::IssueANewIdentity,
    )
    .await;
    assert!(
        matches!(&refused, Err(Error::MarkerMalformed { .. })),
        "expected a malformed marker to be refused even with a new identity asked for, got \
         {refused:?}"
    );
    assert_eq!(
        fs::read(marker_in(&broken)).expect("the marker must still be there"),
        b"not an identity\n",
        "and it was left exactly as it was"
    );
}
