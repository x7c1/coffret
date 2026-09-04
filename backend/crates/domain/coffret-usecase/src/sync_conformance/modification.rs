use coffret_model::{ContainerKind, Generation, Mtime};

use crate::conformance_library::Library;
use crate::entry_paths::entry_path;
use crate::sync::{sync_folders, Surfaced};
use crate::sync_conformance::fixtures::{
    keys, map, master_key, plant, request, touch, write, NEWER, OLDER,
};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A changed file whose Entry lives in a one-file Container gets a replacement,
/// and the old Container leaves the current set.
///
/// The replacement is a new Container with a new ID and not the old one
/// rewritten, and its kind is the kind it replaces (spec: PK-15). One batch
/// carries both halves — the replacement in additions, the replaced Container
/// in removals — which is what lets the Entry Path move from one to the other
/// within a single record (spec: CP-14, PK-12, EP-6). The old object is trashed
/// after the record exists, so it leaves the listing and stays recoverable.
pub async fn a_modified_file_replaces_its_one_file_container(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let path = write(fixture.folder(), "a.jpg", b"the original bytes").await;
    touch(&path, OLDER);
    let first = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a first sync must succeed");
    let original = *first
        .added
        .first()
        .expect("the first sync uploaded the file");

    let changed = b"bytes that are not the original ones".as_slice();
    tokio::fs::write(&path, changed)
        .await
        .expect("rewriting the file must succeed");
    touch(&path, NEWER);

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync after a modification must succeed");

    assert_eq!(outcome.added.len(), 1, "the replacement is a new Container");
    assert_eq!(
        outcome.replaced,
        vec![original],
        "the Container that held the old Entry is what the batch removes",
    );
    assert!(outcome.surfaced.is_empty());
    let replacement = outcome.added[0];
    assert_ne!(
        replacement, original,
        "a replacement has a new Container ID (spec: CP-14, PK-15)",
    );

    let commit = outcome.commit.expect("a modified file is worth a commit");
    assert_eq!(commit.record.generation, Generation::new(1));
    assert_eq!(commit.record.removals, vec![original]);
    assert!(
        commit.untrashed.is_empty(),
        "the replaced Container's object was trashed",
    );

    let location = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the path is still current, held by the replacement");
    assert_eq!(location.container_id, replacement);

    let library = Library::read(store).await;
    assert!(
        !library.holds_container(original),
        "a removed Container's object leaves the listing",
    );
    let container = library
        .open(store, &commit.record, replacement, &master_key())
        .await;
    assert_eq!(container.kind, ContainerKind::OneFile, "spec: PK-15");
    assert_eq!(
        container.entries[0].content, changed,
        "the replacement carries what is on disk now",
    );
}

/// A changed file whose Entry lives in a Pack is reported and left alone.
///
/// Replacing it means read-modify-replace over the whole Pack, which is the
/// half of `update` this flow does not do, so the Pack is not rewritten and no
/// Container is committed (spec: PK-10, PK-11, PK-12). What must not happen is
/// the file being passed over quietly — a scan that selects update candidates
/// surfaces every one of them, because silence here would tell the user that
/// stale content is safely backed up (spec: PK-14).
pub async fn a_pack_resident_change_is_surfaced_and_untouched(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let original = b"what the Pack holds".as_slice();
    let path = write(fixture.folder(), "a.jpg", original).await;
    touch(&path, OLDER);
    let pack = plant(
        store,
        index,
        &keys,
        ContainerKind::Pack,
        "a.jpg",
        original,
        Mtime::from_unix_seconds(OLDER as i64),
        true,
    )
    .await;

    tokio::fs::write(&path, b"bytes the Pack does not hold")
        .await
        .expect("rewriting the file must succeed");
    touch(&path, NEWER);

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync meeting a Pack-resident change must succeed");

    assert!(outcome.commit.is_none(), "no Pack is rewritten by a sync");
    assert!(outcome.added.is_empty());
    assert!(outcome.replaced.is_empty());
    assert_eq!(
        outcome.surfaced,
        vec![Surfaced::PackResident {
            path: entry_path("a.jpg"),
            container_id: pack,
        }],
        "the file needing an update is surfaced (spec: PK-14)",
    );

    let location = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the Entry is still current");
    assert_eq!(
        location.container_id, pack,
        "the Pack still holds the Entry, byte for byte (spec: PK-10)",
    );
    assert!(
        Library::read(store).await.holds_container(pack),
        "the Pack's object is untouched",
    );
}
