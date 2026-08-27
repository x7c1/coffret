use coffret_model::{ContainerKind, EntryPath, Mtime};

use crate::conformance_library::Library;
use crate::sync::{sync_folders, Surfaced};
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

    let path = write(fixture.folder(), "a.jpg", b"the file's bytes").await;
    write(fixture.folder(), "b.jpg", b"a file that stays").await;
    let first = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a first sync must succeed");
    assert_eq!(first.added.len(), 2);
    let gone = index
        .entry_at(&EntryPath::new("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the file this run uploaded is current")
        .container_id;

    tokio::fs::remove_file(&path)
        .await
        .expect("removing a file must succeed");

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync meeting a local deletion must succeed");

    assert!(outcome.commit.is_none(), "a sync propagates no deletion");
    assert_eq!(
        outcome.surfaced,
        vec![Surfaced::DeletedLocally {
            path: EntryPath::new("a.jpg"),
        }],
    );
    assert_eq!(outcome.unchanged, 1, "the file that stayed is unchanged");

    assert!(
        index
            .entry_at(&EntryPath::new("a.jpg"))
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
    let again = sync_folders(request(store, index, &keys, fixture.spool(), 3))
        .await
        .expect("a third sync must succeed");
    assert_eq!(again.surfaced, outcome.surfaced);
}

/// An Entry this device never materialized is left alone, mapping or no
/// mapping.
///
/// A mapping translates Entry Paths into local paths and asserts nothing about
/// what is on disk. A path with a current Entry and no local row is one this
/// device never put there, so a local file standing at it — whatever it holds —
/// is neither a modification of that Entry nor evidence about it: it is never
/// reported as changed, never selected for an update, and never proposed for
/// removal (spec: EP-9, EP-10).
pub async fn an_entry_this_device_never_materialized_is_left_alone(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let held = b"what another device uploaded".as_slice();
    let path = write(fixture.folder(), "a.jpg", held).await;
    touch(&path, OLDER);
    let container = plant(
        store,
        index,
        &keys,
        ContainerKind::OneFile,
        "a.jpg",
        held,
        Mtime::from_unix_seconds(OLDER as i64),
        // The Entry is current and this device never placed it: no local row.
        false,
    )
    .await;

    tokio::fs::write(&path, b"bytes this device happens to have")
        .await
        .expect("rewriting the file must succeed");
    touch(&path, NEWER);

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
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
        .entry_at(&EntryPath::new("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the Entry is still current");
    assert_eq!(location.container_id, container);
    assert!(
        index
            .local_entry_at(&EntryPath::new("a.jpg"))
            .await
            .expect("asking the Index for a local row must succeed")
            .is_none(),
        "a sync that placed nothing invents no claim to have placed it",
    );
}
