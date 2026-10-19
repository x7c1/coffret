use coffret_model::{ContainerKind, Mtime};

use crate::conformance_library::Library;
use crate::entry_paths::entry_path;
use crate::fetch_conformance::fixtures::lose_key;
use crate::generations::generation;
use crate::sync::{sync_folders, Surfaced};
use crate::sync_conformance::fixtures::{
    at, keys, map, master_key, plant, request, touch, write, NEWER, OLDER,
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

    let path = write(
        fixture.fs(),
        fixture.folder(),
        "a.jpg",
        b"the original bytes",
    );
    touch(fixture.fs(), &path, OLDER);
    let first = sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .expect("a first sync must succeed");
    let original = *first
        .added
        .first()
        .expect("the first sync uploaded the file");
    let deposited = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the first sync committed the file")
        .deposited_at;
    assert_eq!(
        deposited.as_unix_seconds(),
        at(1).as_unix_seconds(),
        "an addition is a deposit, stamped with the clock of the run that built its record \
         (spec: FM-15)",
    );

    let changed = b"bytes that are not the original ones".as_slice();
    fixture.fs().write_file(&path, changed);
    touch(fixture.fs(), &path, NEWER);

    let outcome = sync_folders(request(store, index, &keys, fixture.fs(), 2))
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
    assert_eq!(commit.record.generation(), generation(2));
    assert_eq!(commit.record.removals(), vec![original]);
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
    assert_eq!(
        location.deposited_at.as_unix_seconds(),
        at(2).as_unix_seconds(),
        "a replacement is a deposit too, and sets a new time (spec: FM-15, PK-11)",
    );

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

/// A key-lost Entry keeps its deposit time while its file holds the same
/// content, and is deposited anew once the file is rewritten.
///
/// Healing a lost key is no deposit by itself: a deposit is new content, and
/// whether content is new is its hash's to say. So a touched file whose bytes
/// are the Entry's leaves the time where the first sync stamped it, and the
/// replacement a rewrite brings — for which a key-lost Container is always
/// eligible — is stamped with the clock of the run that commits it
/// (spec: FM-15, KL-17, PK-11).
pub async fn a_key_lost_entry_is_deposited_anew_only_for_new_content(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let original = b"the bytes whose key is lost".as_slice();
    let path = write(fixture.fs(), fixture.folder(), "a.jpg", original);
    touch(fixture.fs(), &path, OLDER);
    let first = sync_folders(request(store, index, &keys, fixture.fs(), 1))
        .await
        .expect("a first sync must succeed");
    let lost = *first
        .added
        .first()
        .expect("the first sync uploaded the file");

    // What a device meets after another one rebuilt the Keyring from the
    // material it had, with a marker where the key is gone (spec: RV-8, KL-7).
    lose_key(store, index, lost).await;

    // The same bytes, touched: nothing new reaches the Library.
    touch(fixture.fs(), &path, NEWER);
    let touched = sync_folders(request(store, index, &keys, fixture.fs(), 2))
        .await
        .expect("a sync over a touched file must succeed");
    assert!(touched.added.is_empty() && touched.replaced.is_empty());
    let location = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the key-lost Entry stays current (spec: KL-17)");
    assert_eq!(location.container_id, lost);
    assert_eq!(
        location.deposited_at.as_unix_seconds(),
        at(1).as_unix_seconds(),
        "the same content keeps the time it was deposited at (spec: FM-15)",
    );

    let changed = b"bytes the Library has never held".as_slice();
    fixture.fs().write_file(&path, changed);
    touch(fixture.fs(), &path, OLDER);
    let rewritten = sync_folders(request(store, index, &keys, fixture.fs(), 3))
        .await
        .expect("a sync over a rewritten file must succeed");
    assert_eq!(
        rewritten.replaced,
        vec![lost],
        "a key-lost Container is replaced like any other (spec: KL-17, PK-11)",
    );
    let location = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the path is current, held by the replacement");
    assert_ne!(location.container_id, lost);
    assert_eq!(
        location.deposited_at.as_unix_seconds(),
        at(3).as_unix_seconds(),
        "new content is a deposit and sets a new time (spec: FM-15, PK-11)",
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
    let path = write(fixture.fs(), fixture.folder(), "a.jpg", original);
    touch(fixture.fs(), &path, OLDER);
    let pack = plant(
        store,
        index,
        &keys,
        ContainerKind::Pack,
        "a.jpg",
        original,
        Mtime::from_unix_seconds(OLDER),
        true,
    )
    .await;

    fixture
        .fs()
        .write_file(&path, b"bytes the Pack does not hold");
    touch(fixture.fs(), &path, NEWER);

    let outcome = sync_folders(request(store, index, &keys, fixture.fs(), 2))
        .await
        .expect("a sync meeting a Pack-resident change must succeed");

    assert!(outcome.commit.is_none(), "no Pack is rewritten by a sync");
    assert!(outcome.added.is_empty());
    assert!(outcome.replaced.is_empty());
    assert!(
        matches!(
            outcome.surfaced.as_slice(),
            [Surfaced::PackResident { path, container_id }]
                if *path == entry_path("a.jpg") && *container_id == pack
        ),
        "the file needing an update is surfaced (spec: PK-14): {:?}",
        outcome.surfaced,
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
