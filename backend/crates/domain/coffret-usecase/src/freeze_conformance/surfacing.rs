use coffret_model::{EntryPath, Mtime};

use crate::conformance_library::Library;
use crate::freeze::NotFrozen;
use crate::freeze_conformance::counting_store::CountingStore;
use crate::freeze_conformance::fixtures::{
    at, container_handle, freeze, freeze_against, keys, lose_key, map, opened, touch, write, OLDER,
    TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;

/// A modified file whose Entry a Pack holds is surfaced, and the Pack is
/// untouched.
///
/// An Entry already in a Pack is never eligible: existing Packs are neither read
/// as input nor rewritten by a freeze (spec: PK-1, PK-2). Carrying the change in
/// means read-modify-replace over that Pack, which is `update`'s (spec: PK-10,
/// PK-11). What must not happen is the file being passed over quietly — a scan
/// that selects freeze candidates surfaces every update-eligible file, because
/// silence would tell the user that stale content is safely backed up
/// (spec: PK-14).
///
/// The rest of the folder freezes as usual in the same run, so surfacing is a
/// finding about one file rather than a refusal of the invocation.
pub async fn a_modified_pack_resident_entry_is_surfaced_and_untouched(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let held = b"what the Pack holds".as_slice();
    let path = write(fixture.source_folder(), "albums/a.jpg", held).await;
    touch(&path, OLDER);
    let first = freeze(fixture, &keys, TARGET, 1).await;
    assert_eq!(first.frozen_entries(), 1);
    let pack = first.packs[0].container_id;

    tokio::fs::write(&path, b"bytes the Pack does not hold")
        .await
        .expect("rewriting the file must succeed");
    touch(&path, OLDER + 600);
    // A file the run *can* pack, alongside the one it cannot, so the case shows
    // that surfacing costs one file and not the invocation.
    write(fixture.source_folder(), "albums/b.jpg", b"a new file").await;

    let counting = CountingStore::around(store);
    let outcome = freeze_against(&counting, fixture, &keys, TARGET, 2).await;

    assert_eq!(
        outcome.surfaced,
        vec![NotFrozen::ModifiedInPack {
            path: EntryPath::new("albums/a.jpg"),
            container_id: pack,
        }],
        "the file needing an update is surfaced (spec: PK-14)",
    );
    assert_eq!(
        outcome.frozen_entries(),
        1,
        "the new file is packed all the same",
    );
    assert!(
        outcome.absorbed.is_empty(),
        "an existing Pack never appears in removals (spec: PK-7)",
    );
    assert_eq!(outcome.packed_already, 0);

    let commit = outcome.commit.expect("the new file is worth a commit");
    assert!(!commit.record.removals.contains(&pack));

    let location = index
        .entry_at(&EntryPath::new("albums/a.jpg"))
        .await
        .expect("asking the catalog for a path must succeed")
        .expect("the Entry is still current");
    assert_eq!(
        location.container_id, pack,
        "the Pack still holds the Entry (spec: PK-10)",
    );
    assert!(
        !counting.wrote(pack),
        "the Pack is not rewritten (spec: PK-2)",
    );
    assert!(
        !counting.read_object(&container_handle(store, pack).await),
        "the Pack is not read as input either (spec: PK-1)",
    );
    assert!(Library::read(store).await.holds_container(pack));

    let decoded = opened(store, &commit.record, pack).await;
    assert_eq!(
        decoded.entries[0].content, held,
        "the Pack holds what it always held, byte for byte",
    );
}

/// A Pack-resident file that was touched and not changed is not a finding, and
/// stops being read on every later run.
///
/// The other side of the case above, and the line PK-14 actually draws: what may
/// not be passed over quietly is content that differs from the Entry, not a
/// modification time that moved. The cheap comparison says the file may have
/// changed and the expensive one settles it — the plaintext still hashes to what
/// the Pack holds — so the file is counted as already packed rather than reported
/// (spec: PK-2, PK-14).
///
/// What does happen is that the device writes down what it now sees, so the next
/// run answers from the length and the modification time again instead of hashing
/// the file a second time (spec: EP-10). That bookkeeping is this device's own
/// and lands whether or not the run commits anything — and this run commits
/// nothing, because nothing was eligible (spec: CP-1).
pub async fn a_touched_pack_resident_entry_is_not_a_finding(fixture: &FreezeUnderTest) {
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let held = b"what the Pack holds".as_slice();
    let path = write(fixture.source_folder(), "albums/a.jpg", held).await;
    touch(&path, OLDER);
    let first = freeze(fixture, &keys, TARGET, 1).await;
    assert_eq!(first.frozen_entries(), 1);

    const RESTAMPED: i64 = OLDER + 600;
    touch(&path, RESTAMPED);
    let outcome = freeze(fixture, &keys, TARGET, 2).await;

    assert!(
        outcome.surfaced.is_empty(),
        "equal content is not a finding, however the stamp moved (spec: PK-14)",
    );
    assert_eq!(
        outcome.packed_already, 1,
        "the file is already in a Pack and still matches it (spec: PK-2)",
    );
    assert!(outcome.packs.is_empty(), "there was nothing to pack");
    assert!(outcome.commit.is_none(), "there was nothing to commit");

    let local = index
        .local_entry_at(&EntryPath::new("albums/a.jpg"))
        .await
        .expect("asking the catalog for a local row must succeed")
        .expect("this device placed the file");
    assert_eq!(
        local.observation.mtime,
        Mtime::from_unix_seconds(RESTAMPED),
        "the device wrote down what it now sees, so the next run hashes nothing",
    );
    assert_eq!(
        local.observation.at,
        at(2),
        "stamped with the run that looked, not the one that placed the file",
    );
}

/// An Entry whose Pack cannot be opened is surfaced rather than passed over.
///
/// The same loss PK-13 lets a freeze repair for a one-file Container, met over a
/// Pack instead: the stored ciphertext is unreadable, so the content can only be
/// recovered by re-encrypting the local plaintext, and over a Pack that is
/// read-modify-replace (spec: KL-7, PK-10, PK-11). A freeze does not do it —
/// but it may not stay silent about it either, because silence would tell the
/// user that unrecoverable content is safely backed up (spec: PK-14).
pub async fn a_key_lost_pack_entry_is_surfaced_and_untouched(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let held = b"what the unreadable Pack holds".as_slice();
    let path = write(fixture.source_folder(), "albums/a.jpg", held).await;
    touch(&path, OLDER);
    let first = freeze(fixture, &keys, TARGET, 1).await;
    let pack = first.packs[0].container_id;

    lose_key(store, index, pack).await;

    let outcome = freeze(fixture, &keys, TARGET, 2).await;

    assert_eq!(
        outcome.surfaced,
        vec![NotFrozen::KeyLostInPack {
            path: EntryPath::new("albums/a.jpg"),
            container_id: pack,
        }],
        "an unreadable Pack is reported however the local file compares (spec: PK-14)",
    );
    assert!(outcome.packs.is_empty(), "no Pack was rewritten");
    assert!(outcome.absorbed.is_empty());
    assert!(outcome.commit.is_none(), "there was nothing to commit");
    assert!(Library::read(store).await.holds_container(pack));
}
