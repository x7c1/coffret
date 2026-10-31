use std::collections::BTreeSet;

use coffret_model::{ContainerKind, Mtime};

use crate::conformance_library::Library;
use crate::device_state::Mapping;
use crate::entry_paths::entry_path;
use crate::sync::{sync_folders, ScanScope, Surfaced};
use crate::sync_conformance::fixtures::{keys, map, plant, request, touch, write, NEWER, OLDER};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A file this device placed and no longer has is reported, and nothing is
/// removed.
///
/// A deletion is a fact about a file this device materialized, and reporting it
/// is all a sync does with it: taking the Entry out of the Library is a removal
/// the user asks for explicitly, never one inferred from a missing file
/// (spec: EP-10, CP-14). The device-local row survives untouched, which is what
/// lets the finding be reported again by every later run instead of being
/// forgotten after the first.
pub async fn a_file_deleted_locally_is_surfaced_and_untouched(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let path = write(fixture.fs(), fixture.folder(), "a.jpg", b"the file's bytes");
    write(
        fixture.fs(),
        fixture.folder(),
        "b.jpg",
        b"a file that stays",
    );
    let first = sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .expect("a first sync must succeed");
    assert_eq!(first.added.len(), 2);
    let gone = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the file this run uploaded is current")
        .container_id;

    fixture.fs().remove_file(&path);

    let outcome = sync_folders(request(store, index, &keys, fixture.fs(), 2))
        .await
        .expect("a sync meeting a local deletion must succeed");

    assert!(outcome.commit.is_none(), "a sync propagates no deletion");
    assert!(
        matches!(
            outcome.surfaced.as_slice(),
            [Surfaced::DeletedLocally { path }] if *path == entry_path("a.jpg")
        ),
        "the missing file is reported: {:?}",
        outcome.surfaced,
    );
    assert_eq!(outcome.unchanged, 1, "the file that stayed is unchanged");

    assert!(
        index
            .entry_at(&entry_path("a.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "the Entry is still current: nothing removed it",
    );
    assert!(
        Library::read(store).await.holds_container(gone),
        "the Container holding it is still in Storage",
    );

    // Reported again next time, because nothing about the row changed.
    let again = sync_folders(request(store, index, &keys, fixture.fs(), 3))
        .await
        .expect("a third sync must succeed");
    assert!(
        matches!(
            again.surfaced.as_slice(),
            [Surfaced::DeletedLocally { path }] if *path == entry_path("a.jpg")
        ),
        "the missing file is reported again: {:?}",
        again.surfaced,
    );
}

/// An Entry this device never materialized is left alone, mapping or no
/// mapping.
///
/// A mapping translates Entry Paths into local paths and asserts nothing about
/// what is on disk. A path with a current Entry and no local row is one this
/// device never put there, so a local file standing at it — whatever it holds —
/// is neither a modification of that Entry nor evidence about it: it is never
/// reported as modified, never selected for an update, and never proposed for
/// removal (spec: EP-9, EP-10).
pub async fn an_entry_this_device_never_materialized_is_left_alone(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let held = b"what another device uploaded".as_slice();
    let path = write(fixture.fs(), fixture.folder(), "a.jpg", held);
    touch(fixture.fs(), &path, OLDER);
    let container = plant(
        store,
        index,
        &keys,
        ContainerKind::OneFile,
        "a.jpg",
        held,
        Mtime::from_unix_seconds(OLDER),
        // The Entry is current and this device never placed it: no local row.
        false,
    )
    .await;

    fixture
        .fs()
        .write_file(&path, b"bytes this device happens to have");
    touch(fixture.fs(), &path, NEWER);

    let outcome = sync_folders(request(store, index, &keys, fixture.fs(), 2))
        .await
        .expect("a sync over an Entry outside this device's scope must succeed");

    assert!(outcome.commit.is_none());
    assert!(outcome.added.is_empty());
    assert!(outcome.replaced.is_empty());
    assert!(
        outcome.surfaced.is_empty(),
        "an Entry the device never held is outside its scope, not a finding",
    );

    let location = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the Entry is still current");
    assert_eq!(location.container_id, container);
    assert!(
        index
            .local_entry_at(&entry_path("a.jpg"))
            .await
            .expect("asking the Index for a local row must succeed")
            .is_none(),
        "a sync that placed nothing invents no claim to have placed it",
    );
}

/// A sync narrowed to some folders walks those alone, and a sync of the
/// default request walks every mapping (spec: EP-10).
///
/// The narrowed one is what a sync a drop armed is: it carries in the files
/// standing directly in the folders the drop wrote into, and looks nowhere
/// else — not into a folder beside them, not into one below them, and not
/// under a mapping that reaches none of them, whose root it does not even ask
/// about. So a file this device placed and lost out there is not reported as
/// deleted, and a new file out there is not carried in. The next sync of
/// every mapping does both.
pub async fn a_sync_within_some_folders_walks_those_alone(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    let remainder = fixture.folder().join("everything");
    let albums = fixture.folder().join("photographs");
    for (prefix, local_root) in [
        (None, remainder.clone()),
        (Some(entry_path("albums")), albums.clone()),
    ] {
        index
            .set_mapping(Mapping::new(prefix, local_root))
            .await
            .expect("recording a mapping must succeed");
    }
    let lost = write(
        fixture.fs(),
        &remainder,
        "other/lost.jpg",
        b"placed, then lost",
    );
    write(fixture.fs(), &albums, "spring.jpg", b"a photo");
    let first = sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .expect("a first sync must succeed");
    assert_eq!(first.added.len(), 2);

    fixture.fs().remove_file(&lost);
    write(fixture.fs(), &remainder, "dropped/a.jpg", b"dropped");
    write(
        fixture.fs(),
        &remainder,
        "dropped/below/b.jpg",
        b"below the drop",
    );
    write(
        fixture.fs(),
        &remainder,
        "other/new.jpg",
        b"put there by hand",
    );
    write(fixture.fs(), &remainder, "top.jpg", b"at the top");
    // The `albums` mapping's root goes, which a sync that asked about it would
    // report (spec: EP-12).
    fixture.fs().remove_dir_all(&albums);

    let narrowed = sync_folders(request(store, index, &keys, fixture.fs(), 2).within(
        ScanScope::Folders {
            root: false,
            folders: BTreeSet::from([entry_path("dropped")]),
        },
    ))
    .await
    .expect("a narrowed sync must succeed");

    assert_eq!(narrowed.added.len(), 1, "{narrowed:?}");
    assert!(index
        .entry_at(&entry_path("dropped/a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .is_some());
    for path in ["dropped/below/b.jpg", "other/new.jpg", "top.jpg"] {
        assert!(
            index
                .entry_at(&entry_path(path))
                .await
                .expect("asking the Index for a path must succeed")
                .is_none(),
            "{path} stands outside the folders the sync covers",
        );
    }
    assert!(
        narrowed.surfaced.is_empty(),
        "a file outside the folders is not read as deleted: {:?}",
        narrowed.surfaced,
    );
    assert!(
        narrowed.unavailable.is_empty(),
        "a mapping that reaches none of the folders is not asked about",
    );
    assert_eq!(narrowed.mappings, 2, "the device still maps both");

    let full = sync_folders(request(store, index, &keys, fixture.fs(), 3))
        .await
        .expect("a sync of every mapping must succeed");
    assert_eq!(
        full.added.len(),
        3,
        "everything the narrowed sync left: {full:?}"
    );
    assert!(
        matches!(
            full.surfaced.as_slice(),
            [Surfaced::DeletedLocally { path }] if *path == entry_path("other/lost.jpg")
        ),
        "{:?}",
        full.surfaced,
    );
    assert_eq!(
        full.unavailable.len(),
        1,
        "and the root that went is reported"
    );
}
