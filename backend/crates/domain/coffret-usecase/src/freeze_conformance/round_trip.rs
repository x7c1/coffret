use crate::entry_paths::entry_path;
use crate::fetch::{fetch_folders, FetchRequest};
use crate::freeze_conformance::fixtures::{
    at, filler, freeze, hash, keys, map, policy, read, write, ROOMY_TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;

/// A folder one device froze arrives, byte for byte, in another device's folder.
///
/// This is the round trip Packs have to survive, and it is asserted from what is
/// on disk rather than from what either run returned. The fetching device starts
/// with an empty catalog, so its catch-up is a real restore-and-replay
/// (spec: CK-9, RV-1) — and everything after it follows from control state
/// alone: the committed Keyring opens under a purpose key derived from the
/// Master Key, the envelope it maps each Pack to unwraps against that Pack's own
/// ID, and the object decodes to the files that were on the other device's disk
/// (spec: RV-2, RV-3, KL-7, FM-14).
///
/// It also pins what packing is *for*. The fetch unit is a whole Container
/// however many of its Entries are wanted (spec: PK-16), so a folder held in a
/// handful of Packs costs a handful of fetches rather than one per file — which
/// is the object-count band the pack policy exists to hold.
pub async fn a_second_device_fetches_a_frozen_folder(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    let files: Vec<(String, Vec<u8>)> = (0..11)
        .map(|index| {
            (
                format!("albums/2026/{index:03}.jpg"),
                filler(90 + index * 3, 0x90 + index as u8),
            )
        })
        .collect();
    for (relative, content) in &files {
        write(fixture.source_folder(), relative, content).await;
    }

    let frozen = freeze(fixture, &keys, ROOMY_TARGET, 1).await;
    assert!(frozen.packs.len() > 1, "the folder is cut more than once");
    assert_eq!(frozen.frozen_entries(), files.len());

    let outcome = fetch_folders(
        FetchRequest::new(store, fixture.target(), &keys, at(2)).with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("a fetch by a second device must succeed: {error}"));

    assert_eq!(
        outcome.fetched,
        files
            .iter()
            .map(|(relative, _)| entry_path(relative.clone()))
            .collect::<Vec<_>>(),
        "every Entry, in the order the Library puts them in (spec: EP-3)",
    );
    assert_eq!(
        outcome.containers.len(),
        frozen.packs.len(),
        "one fetch per Pack, however many Entries each held (spec: PK-16)",
    );
    assert!(
        outcome.containers.len() < files.len(),
        "which is the whole point: fewer fetches than files",
    );
    assert!(outcome.surfaced.is_empty());
    assert!(outcome.locked.is_empty());
    assert_eq!(outcome.skipped, 0);

    // What is actually on the second device's disk, which is the only thing a
    // round trip is worth.
    for (relative, content) in &files {
        let placed = read(&fixture.target_folder().join(relative)).await;
        assert_eq!(
            hash(&placed),
            hash(content),
            "{relative} arrived as the file that left",
        );
        assert_eq!(&placed, content);
        assert!(
            fixture
                .target()
                .local_entry_at(&entry_path(relative.clone()))
                .await
                .expect("asking the target catalog for a local row must succeed")
                .is_some(),
            "{relative} is now this device's own materialization (spec: EP-10)",
        );
    }
}
