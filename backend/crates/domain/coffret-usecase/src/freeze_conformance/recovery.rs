use coffret_model::ContainerKind;

use crate::conformance_library::Library;
use crate::entry_paths::entry_path;
use crate::freeze_conformance::fixtures::{
    at, freeze, keys, lose_key, map, opened, sync_source, touch, write, OLDER, TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;
use crate::index::Index;

/// The bytes the first sync put in the Library.
const ORIGINAL: &[u8] = b"the bytes the sync committed";

/// The bytes on disk when the freeze runs.
const CHANGED: &[u8] = b"bytes the Library does not hold yet";

/// A modified file whose Entry is in a one-file Container freezes to the local
/// bytes.
///
/// The overlap PK-13 names: such a file is eligible for both `update` and
/// `freeze`, and either path uploads the content on disk now — the freeze
/// additionally regrouping it into a Pack. So this is one run doing both jobs,
/// and the assertion is that the Pack carries what is on disk rather than what
/// the Library held, and that the old Container leaves the current set
/// (spec: PK-1, PK-7, PK-13).
pub async fn a_modified_one_file_entry_freezes_to_the_local_bytes(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let path = write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/a.jpg",
        ORIGINAL,
    );
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/b.jpg",
        b"a file that does not change",
    );
    touch(fixture.fs(), &path, OLDER);
    let synced = sync_source(fixture, &keys, 1).await;
    assert_eq!(synced.added.len(), 2);
    let original = index_container(index, "albums/a.jpg").await;

    fixture.fs().write_file(&path, CHANGED);
    touch(fixture.fs(), &path, OLDER + 600);

    let outcome = freeze(fixture, &keys, TARGET, 2).await;

    assert_eq!(outcome.frozen_entries(), 2);
    assert!(
        outcome.absorbed.contains(&original),
        "the Container holding the stale Entry is absorbed (spec: PK-7)",
    );
    assert!(
        outcome.surfaced.is_empty(),
        "a one-file Container is a freeze's to absorb, not to surface (spec: PK-13)",
    );

    let commit = outcome.commit.expect("a modified file is worth a commit");
    let pack = index_container(index, "albums/a.jpg").await;
    assert_ne!(pack, original, "the Pack is a new Container (spec: CP-14)");

    let decoded = opened(store, &commit.record, pack).await;
    assert_eq!(decoded.kind, ContainerKind::Pack, "spec: PK-15");
    let entry = decoded
        .entries
        .iter()
        .find(|entry| entry.metadata.path.as_str() == "albums/a.jpg")
        .expect("the Pack holds the Entry the catalog names");
    assert_eq!(
        entry.content, CHANGED,
        "the Pack carries what is on disk now (spec: PK-13)",
    );
    assert!(
        !Library::read(store).await.holds_container(original),
        "the absorbed Container's object leaves the listing",
    );

    // The changed file puts new content into the Library, as the `update` it
    // overlaps with would, so it is deposited at the freeze's clock; the file
    // that did not change is carried into the Pack and keeps the time the sync
    // deposited it at (spec: PK-13, FM-15).
    for (relative, run) in [("albums/a.jpg", 2), ("albums/b.jpg", 1)] {
        let deposited = index
            .entry_at(&entry_path(relative))
            .await
            .expect("asking the catalog for a path must succeed")
            .expect("the Entry is current")
            .deposited_at;
        assert_eq!(
            deposited.as_unix_seconds(),
            at(run).as_unix_seconds(),
            "{relative} is deposited at run {run}",
        );
    }
}

/// A one-file Entry whose Container's key is lost freezes to the local bytes.
///
/// Under a lost key the stored ciphertext cannot be read at all, so
/// re-encrypting the surviving local plaintext into a replacement is the only
/// content-recovery path there is (spec: PK-11, KL-7). PK-13 says the same
/// overlap holds here as for a modification: `freeze` may take it, and takes it
/// from the local file. And it takes it whether or not the content differs —
/// which is exactly what this case pins, because one file is untouched, so a
/// freeze that decided eligibility from a content comparison would pass it over
/// and leave the Library holding an Entry nobody can ever open.
///
/// Healing the loss is no deposit by itself: whether the replacement puts new
/// content into the Library is its hash's to say. So the untouched file keeps
/// the time the sync deposited it at, and the one rewritten after its key was
/// lost is deposited at the freeze's clock (spec: FM-15, KL-17, PK-13).
pub async fn a_key_lost_one_file_entry_freezes_to_the_local_bytes(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let kept = write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/a.jpg",
        ORIGINAL,
    );
    touch(fixture.fs(), &kept, OLDER);
    let rewritten = write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/b.jpg",
        ORIGINAL,
    );
    touch(fixture.fs(), &rewritten, OLDER);
    let synced = sync_source(fixture, &keys, 1).await;
    assert_eq!(synced.added.len(), 2);
    let original = index_container(index, "albums/a.jpg").await;
    let other = index_container(index, "albums/b.jpg").await;

    // What a device meets after another one rebuilt the Keyring from the
    // material it had, with a marker where each key is gone (spec: RV-8, KL-7).
    // The sync between the two losses is what catches this device up to the
    // first, so the second is written over the head it moved to.
    lose_key(store, index, original).await;
    sync_source(fixture, &keys, 2).await;
    lose_key(store, index, other).await;
    write(
        fixture.fs(),
        fixture.source_folder(),
        "albums/b.jpg",
        CHANGED,
    );

    let outcome = freeze(fixture, &keys, TARGET, 3).await;

    assert_eq!(
        outcome.frozen_entries(),
        2,
        "an unreadable one-file Container is eligible however its content compares \
         (spec: PK-11, PK-13)",
    );
    let mut absorbed = outcome.absorbed.clone();
    absorbed.sort();
    let mut expected = vec![original, other];
    expected.sort();
    assert_eq!(absorbed, expected);
    assert!(outcome.surfaced.is_empty());

    let commit = outcome
        .commit
        .expect("recovering an unreadable Entry is worth a commit");
    let pack = index_container(index, "albums/a.jpg").await;
    let decoded = opened(store, &commit.record, pack).await;
    assert_eq!(decoded.kind, ContainerKind::Pack);
    let content = |relative: &str| {
        decoded
            .entries
            .iter()
            .find(|entry| entry.metadata.path == entry_path(relative))
            .unwrap_or_else(|| panic!("the Pack holds {relative:?}"))
            .content
            .clone()
    };
    assert_eq!(
        content("albums/a.jpg"),
        ORIGINAL,
        "the Pack carries the plaintext that survived on disk",
    );
    assert_eq!(content("albums/b.jpg"), CHANGED);
    for container in [original, other] {
        assert!(
            !Library::read(store).await.holds_container(container),
            "the unreadable Container leaves the listing",
        );
    }

    for (relative, run) in [("albums/a.jpg", 1), ("albums/b.jpg", 3)] {
        let deposited = index
            .entry_at(&entry_path(relative))
            .await
            .expect("asking the catalog for a path must succeed")
            .expect("the Entry is current")
            .deposited_at;
        assert_eq!(
            deposited.as_unix_seconds(),
            at(run).as_unix_seconds(),
            "{relative} is deposited at run {run}",
        );
    }
}

/// Which Container the catalog says holds one path's current Entry.
async fn index_container(index: &dyn Index, path: &str) -> coffret_model::ContainerId {
    index
        .entry_at(&entry_path(path))
        .await
        .expect("asking the catalog for a path must succeed")
        .unwrap_or_else(|| panic!("{path:?} must be a current Entry"))
        .container_id
}
