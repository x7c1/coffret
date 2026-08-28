use coffret_model::{ContainerKind, EntryPath};

use crate::conformance_library::Library;
use crate::freeze_conformance::fixtures::{
    freeze, keys, lose_key, map, opened, sync_source, touch, write, OLDER, TARGET,
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

    let path = write(fixture.source_folder(), "albums/a.jpg", ORIGINAL).await;
    write(
        fixture.source_folder(),
        "albums/b.jpg",
        b"a file that does not change",
    )
    .await;
    touch(&path, OLDER);
    let synced = sync_source(fixture, &keys, 1).await;
    assert_eq!(synced.added.len(), 2);
    let original = index_container(index, "albums/a.jpg").await;

    tokio::fs::write(&path, CHANGED)
        .await
        .expect("rewriting the file must succeed");
    touch(&path, OLDER + 600);

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
}

/// A one-file Entry whose Container's key is lost freezes to the local bytes.
///
/// Under a lost key the stored ciphertext cannot be read at all, so
/// re-encrypting the surviving local plaintext into a replacement is the only
/// content-recovery path there is (spec: PK-11, KL-7). PK-13 says the same
/// overlap holds here as for a modification: `freeze` may take it, and takes it
/// from the local file. And it takes it whether or not the content differs —
/// which is exactly what this case pins, because the file is untouched, so a
/// freeze that decided eligibility from a content comparison would pass it over
/// and leave the Library holding an Entry nobody can ever open.
pub async fn a_key_lost_one_file_entry_freezes_to_the_local_bytes(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let path = write(fixture.source_folder(), "albums/a.jpg", ORIGINAL).await;
    touch(&path, OLDER);
    let synced = sync_source(fixture, &keys, 1).await;
    assert_eq!(synced.added.len(), 1);
    let original = index_container(index, "albums/a.jpg").await;

    // What a device meets after another one rebuilt the Keyring from the
    // material it had, with a marker where the key is gone (spec: RV-8, KL-7).
    lose_key(store, index, original).await;

    let outcome = freeze(fixture, &keys, TARGET, 2).await;

    assert_eq!(
        outcome.frozen_entries(),
        1,
        "an unreadable one-file Container is eligible however its content compares \
         (spec: PK-11, PK-13)",
    );
    assert_eq!(outcome.absorbed, vec![original]);
    assert!(outcome.surfaced.is_empty());

    let commit = outcome
        .commit
        .expect("recovering an unreadable Entry is worth a commit");
    let pack = index_container(index, "albums/a.jpg").await;
    let decoded = opened(store, &commit.record, pack).await;
    assert_eq!(decoded.kind, ContainerKind::Pack);
    assert_eq!(
        decoded.entries[0].content, ORIGINAL,
        "the Pack carries the plaintext that survived on disk",
    );
    assert!(
        !Library::read(store).await.holds_container(original),
        "the unreadable Container leaves the listing",
    );
}

/// Which Container the catalog says holds one path's current Entry.
async fn index_container(index: &dyn Index, path: &str) -> coffret_model::ContainerId {
    index
        .entry_at(&EntryPath::nfc(path))
        .await
        .expect("asking the catalog for a path must succeed")
        .unwrap_or_else(|| panic!("{path:?} must be a current Entry"))
        .container_id
}
