use std::path::{Path, PathBuf};

use coffret_model::EntryPath;

use crate::device_state::{LocalEntryState, Mapping};
use crate::index::Index;
use crate::sync::{sync_folders, RootUnavailable, Surfaced, SyncOutcome, UnavailableRoot};
use crate::sync_conformance::fixtures::{
    another_filesystem, keys, map_at, map_with, mappings, request, write,
};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A mapped root that is not there is reported, and nothing under it is inferred
/// gone.
///
/// An unplugged external drive, or a network mount that went away, leaves a
/// mapped root that cannot be opened at all. Reading that as a folder holding
/// nothing would report every Entry the device placed under it as deleted — the
/// whole subtree, on a run the user started for another reason entirely. So the
/// mapping is reported and the subtree is left alone: nothing is walked under it,
/// no deletion is inferred, and no row is written down (spec: EP-12). The
/// device's other mapping scans exactly as it would have.
pub async fn a_missing_mapped_root_is_reported_and_infers_no_deletion(fixture: &SyncUnderTest) {
    let index = fixture.index();
    let (_remainder, albums) = two_roots(fixture).await;

    let first = sync(fixture, 1).await;
    assert_eq!(first.added.len(), 2, "one file under each mapping");
    assert!(first.unavailable.is_empty(), "both roots were there");

    remove_root(&albums).await;

    let outcome = sync(fixture, 2).await;

    assert_eq!(
        outcome.unavailable,
        vec![UnavailableRoot {
            prefix: Some(EntryPath::new("albums")),
            local_root: albums,
            reason: RootUnavailable::Missing,
        }],
    );
    assert!(
        outcome.surfaced.is_empty(),
        "a root the device cannot vouch for infers no deletion",
    );
    assert!(outcome.commit.is_none(), "nothing changed in the Library");
    assert_eq!(
        outcome.unchanged, 1,
        "the mapping whose root is there scanned normally",
    );

    assert_current(index, "albums/spring.jpg").await;
    assert_eq!(
        present_state(index, "albums/spring.jpg").await,
        LocalEntryState::Present,
        "the local row is untouched: the run wrote nothing down about the file",
    );
    // Nothing was walked under the unavailable root, so the mapping keeps the
    // identity the first run stamped it with.
    assert!(mappings(index)
        .await
        .iter()
        .all(|mapping| mapping.root_identity.is_some()));
}

/// A root that is empty and stands on a filesystem the mapping does not record
/// is reported the same way.
///
/// This is the worse shape, and the one a missing-root check alone does not
/// catch: an unmounted mount point is an ordinary empty directory, so the
/// listing succeeds and answers with nothing — the same value a folder the user
/// really emptied gives. What tells them apart is the filesystem identity a
/// scan stamped the mapping with while the disk was there (spec: EP-12).
pub async fn an_empty_root_on_another_filesystem_is_reported_and_infers_no_deletion(
    fixture: &SyncUnderTest,
) {
    let index = fixture.index();
    let root = fixture.folder().join("photographs");
    map_at(fixture, None, &root).await;
    let file = write(&root, "spring.jpg", b"a photo").await;

    let first = sync(fixture, 1).await;
    assert_eq!(first.added.len(), 1);

    unmounted(fixture, &root).await;
    tokio::fs::remove_file(&file)
        .await
        .expect("removing a file must succeed");

    let outcome = sync(fixture, 2).await;

    assert_eq!(
        outcome.unavailable,
        vec![UnavailableRoot {
            prefix: None,
            local_root: root.clone(),
            reason: RootUnavailable::AnotherFilesystem,
        }],
    );
    assert!(
        outcome.surfaced.is_empty(),
        "an unmounted disk is not the user having emptied the folder",
    );
    assert!(outcome.commit.is_none());
    assert_current(index, "spring.jpg").await;
    assert_eq!(
        mappings(index).await,
        vec![Mapping {
            prefix: None,
            local_root: root,
            root_identity: Some(another_filesystem()),
        }],
        "an empty root is never re-stamped, so the recorded identity is as the run found it",
    );
}

/// The legitimate mass delete still reports every deletion it did before.
///
/// The guard would be worthless if it swallowed the behavior it guards: a folder
/// the user really emptied, standing on the filesystem the mapping records, is
/// exactly what a local deletion looks like and is reported as one
/// (spec: EP-10). Removing the root *directory* afterwards makes the same folder
/// the other answer, which is the pair the rule distinguishes (spec: EP-12).
pub async fn an_emptied_folder_on_the_recorded_filesystem_still_reports_its_deletions(
    fixture: &SyncUnderTest,
) {
    let root = fixture.folder().join("photographs");
    map_at(fixture, None, &root).await;
    let spring = write(&root, "spring.jpg", b"a photo").await;
    let summer = write(&root, "summer.jpg", b"another photo").await;

    let first = sync(fixture, 1).await;
    assert_eq!(first.added.len(), 2);
    assert!(
        mappings(fixture.index()).await[0].root_identity.is_some(),
        "the first scan stamped the mapping with the filesystem it saw",
    );

    for file in [&spring, &summer] {
        tokio::fs::remove_file(file)
            .await
            .expect("removing a file must succeed");
    }

    let emptied = sync(fixture, 2).await;
    assert!(
        emptied.unavailable.is_empty(),
        "the root is there and on the filesystem the mapping records",
    );
    assert_eq!(
        emptied.surfaced,
        vec![
            Surfaced::DeletedLocally {
                path: EntryPath::new("spring.jpg"),
            },
            Surfaced::DeletedLocally {
                path: EntryPath::new("summer.jpg"),
            },
        ],
        "an emptied folder reports every deletion, in Entry Path order",
    );

    remove_root(&root).await;

    let gone = sync(fixture, 3).await;
    assert!(
        gone.surfaced.is_empty(),
        "the root itself is gone, so nothing under it is evidence any more",
    );
    assert_eq!(
        gone.unavailable,
        vec![UnavailableRoot {
            prefix: None,
            local_root: root,
            reason: RootUnavailable::Missing,
        }],
    );
}

/// A root whose filesystem identity moved while it still holds files is
/// re-stamped and scans normally.
///
/// Device numbers are not stable across reboots for every filesystem — network
/// mounts, LVM volumes, and btrfs subvolumes can all come back renumbered — and
/// reporting such a root unavailable on every run thereafter would be a folder
/// silently stopping being backed up. So the two conditions are asymmetric
/// deliberately: an empty root whose identity differs is a verdict, and a root
/// holding files whose identity differs costs one re-stamp and nothing else
/// (spec: EP-12).
pub async fn a_renumbered_root_that_holds_files_is_restamped_and_scans_normally(
    fixture: &SyncUnderTest,
) {
    let index = fixture.index();
    let root = fixture.folder().join("photographs");
    write(&root, "spring.jpg", b"a photo").await;
    map_with(fixture, None, &root, Some(another_filesystem())).await;

    let outcome = sync(fixture, 1).await;
    assert!(
        outcome.unavailable.is_empty(),
        "a root that holds files is available whatever its identity says",
    );
    assert_eq!(outcome.added.len(), 1, "the file was committed");

    let stamped = mappings(index).await;
    assert_eq!(stamped.len(), 1);
    let identity = stamped[0]
        .root_identity
        .as_ref()
        .expect("the run stamped the mapping with what it saw");
    assert_ne!(
        identity,
        &another_filesystem(),
        "the stamp is the filesystem the root stands on, not the one recorded before",
    );

    let second = sync(fixture, 2).await;
    assert!(second.unavailable.is_empty(), "the stamp stuck");
    assert_eq!(second.unchanged, 1);
    assert!(second.surfaced.is_empty());
}

/// An unavailable top-level mapping holds its subtree back from the Library-root
/// mapping too.
///
/// Skipping the unavailable mapping's own pass is not enough. The root mapping
/// asks the port for what this device has under *no* prefix, which is every
/// present row it holds — the rows under the other mapping's prefix included —
/// and none of them is in what the walk found. So the deletion step applies the
/// same partition the walk applies: a top-level mapping represents its subtree
/// and the root mapping represents the remainder, whether or not that mapping's
/// disk is plugged in (spec: EP-9, EP-12).
pub async fn an_unavailable_top_level_mapping_holds_its_subtree_back_from_the_root_mapping(
    fixture: &SyncUnderTest,
) {
    let (remainder, albums) = two_roots(fixture).await;

    let first = sync(fixture, 1).await;
    assert_eq!(first.added.len(), 2);

    remove_root(&albums).await;

    let outcome = sync(fixture, 2).await;
    assert_eq!(
        outcome.unavailable,
        vec![UnavailableRoot {
            prefix: Some(EntryPath::new("albums")),
            local_root: albums,
            reason: RootUnavailable::Missing,
        }],
    );
    assert!(
        outcome.surfaced.is_empty(),
        "the root mapping's whole-namespace answer is bounded to the remainder",
    );

    // And the remainder is still the remainder: the root mapping reports its own
    // deletion, and only its own.
    tokio::fs::remove_file(remainder.join("notes.txt"))
        .await
        .expect("removing a file must succeed");

    let third = sync(fixture, 3).await;
    assert_eq!(
        third.surfaced,
        vec![Surfaced::DeletedLocally {
            path: EntryPath::new("notes.txt"),
        }],
        "the file the available mapping lost, and nothing from under the other prefix",
    );
    assert_eq!(third.unavailable.len(), 1);
}

/// Recording the mapping again clears its identity, and the next run reports the
/// deletions.
///
/// The one state the design leaves for a person is a folder genuinely emptied
/// whose filesystem identity also moved: it reports unavailable and keeps doing
/// so, because an empty root is never re-stamped. The gesture that resolves it is
/// the same operation that created the mapping — recorded afresh, so it carries
/// no identity, so the next scan stamps whatever is there and infers the
/// deletions (spec: EP-12).
pub async fn a_mapping_recorded_afresh_clears_its_identity_and_reports_the_deletions(
    fixture: &SyncUnderTest,
) {
    let root = fixture.folder().join("photographs");
    map_at(fixture, None, &root).await;
    let file = write(&root, "spring.jpg", b"a photo").await;

    sync(fixture, 1).await;
    unmounted(fixture, &root).await;
    tokio::fs::remove_file(&file)
        .await
        .expect("removing a file must succeed");

    let stuck = sync(fixture, 2).await;
    assert_eq!(stuck.unavailable.len(), 1, "the root reports unavailable");
    assert!(stuck.surfaced.is_empty());

    // The device says: this root is what I meant.
    map_at(fixture, None, &root).await;

    let outcome = sync(fixture, 3).await;
    assert!(
        outcome.unavailable.is_empty(),
        "a mapping with no recorded identity is guarded by the missing-root check alone",
    );
    assert_eq!(
        outcome.surfaced,
        vec![Surfaced::DeletedLocally {
            path: EntryPath::new("spring.jpg"),
        }],
        "the file that really is gone is reported",
    );
}

/// Two local roots side by side, one at the Library root and one at `albums`,
/// each holding one file.
///
/// Side by side rather than nested, so removing one leaves the other where it
/// was — and subdirectories of the fixture's folder rather than the folder
/// itself, because a case that removes a mapped root must not take the fixture's
/// own directory with it.
async fn two_roots(fixture: &SyncUnderTest) -> (PathBuf, PathBuf) {
    let remainder = fixture.folder().join("everything");
    let albums = fixture.folder().join("photographs");
    map_at(fixture, None, &remainder).await;
    map_at(fixture, Some("albums"), &albums).await;

    write(&remainder, "notes.txt", b"part of the remainder").await;
    write(&albums, "spring.jpg", b"a photo").await;
    (remainder, albums)
}

/// Re-records a mapping with an identity no filesystem this case runs on has,
/// which is what an unmounted disk looks like to the comparison (spec: EP-12).
async fn unmounted(fixture: &SyncUnderTest, root: &Path) {
    map_with(fixture, None, root, Some(another_filesystem())).await;
}

/// Removes a mapped root and everything under it, as unplugging the disk it
/// stood on would.
async fn remove_root(root: &Path) {
    tokio::fs::remove_dir_all(root)
        .await
        .expect("removing a mapped root must succeed");
}

/// One sync run over the case's mappings, which the case expects to succeed.
async fn sync(fixture: &SyncUnderTest, run: i64) -> SyncOutcome {
    let keys = keys();
    sync_folders(request(
        fixture.store(),
        fixture.index(),
        &keys,
        fixture.spool(),
        run,
    ))
    .await
    .unwrap_or_else(|error| panic!("a sync over a mapped folder must succeed: {error}"))
}

/// Asserts that the Library still holds a current Entry at one path.
async fn assert_current(index: &dyn Index, path: &str) {
    assert!(
        index
            .entry_at(&EntryPath::new(path))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "{path} is still current: nothing removed it",
    );
}

/// What this device's own row says about one path.
async fn present_state(index: &dyn Index, path: &str) -> LocalEntryState {
    index
        .local_entry_at(&EntryPath::new(path))
        .await
        .expect("asking the Index for a local row must succeed")
        .expect("this device placed the file, so it has a row for it")
        .state
}
