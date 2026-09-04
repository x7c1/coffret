use coffret_model::{ContainerKind, EntryPath, Generation, Mtime};

use crate::in_memory_index::InMemoryIndex;
use crate::sync::sync_folders;
use crate::sync_conformance::fixtures::{
    keys, map, pending, plant, request, spooled, touch, write, OLDER,
};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// The bytes the Library already holds, which are the bytes on this device.
const COMMITTED: &[u8] = b"the bytes the Library already holds";

/// A sync over a catalog standing behind the Library's head uploads nothing the
/// Library already holds.
///
/// This is the state a device is left in when the catalog half of its Index is
/// discarded — a file from an older layout keeps this device's own state and
/// loses everything the Library can hand back (spec: RV-5) — and, less
/// dramatically, the state any device is in once another one commits while it
/// is not looking. The catalog holds no Entry at the path, the Library's head
/// does, and the file under the mapping is the file the Library holds there.
///
/// Read as it stands, that catalog says the local file is new. The run would
/// spool it, upload it, and then meet the Entry already current at that path as
/// a collision when the commit caught the Index up — every mapped file at once,
/// on a catalog that was discarded whole (spec: EP-6). So the catch-up comes
/// before the scan, and what the scan then reads is the Library as it is: a
/// current Entry at that path, which this device has no local row for and so
/// leaves alone (spec: EP-10). Nothing is uploaded, no generation is spent
/// (spec: CP-1), and the catalog is left standing at the Library's head.
///
/// The Index here carries no local row for that path, where a discard would
/// keep one: with the kept row saying this device materialized the file, the
/// scan lets it be as unchanged instead. Either reading leaves the Library
/// alone, and this case pins the one an Index with no local rows takes.
pub async fn sync_catches_up_before_scanning(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let path = write(fixture.folder(), "a.jpg", COMMITTED).await;
    touch(&path, OLDER);
    // Committed through a catalog of its own, so the Library has a head that
    // this device's Index knows nothing about — which is what a discarded
    // catalog, or another device's commit, leaves behind.
    plant(
        store,
        &InMemoryIndex::new(),
        &keys,
        ContainerKind::OneFile,
        "a.jpg",
        COMMITTED,
        Mtime::from_unix_seconds(OLDER as i64),
        false,
    )
    .await;
    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "the device's catalog stands at no committed state at all",
    );

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a sync over a catalog behind the head must succeed");

    assert!(
        outcome.added.is_empty(),
        "the Library already holds the file, so nothing goes up",
    );
    assert!(
        outcome.commit.is_none(),
        "nothing was uploaded, so no generation is spent (spec: CP-1)",
    );
    assert!(outcome.surfaced.is_empty());
    assert_eq!(spooled(fixture.spool()).await, 0);
    assert!(pending(index).await.is_empty());

    let checkpoint = index
        .checkpoint()
        .await
        .expect("reading the checkpoint must succeed")
        .expect("the run caught the catalog up before it scanned (spec: CK-9)");
    assert_eq!(
        checkpoint.head_generation,
        Generation::FIRST,
        "the catalog stands at the head the store holds, and the run moved it no further",
    );
    assert!(
        index
            .entry_at(&EntryPath::nfc("a.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "the Entry the Library holds at the path is current in the catalog",
    );
}
