use std::path::Path;

use crate::catch_up::{catch_up_catalog, CatchUpRequest};
use crate::conformance_library::Library;
use crate::delete::{committed_key_lost, preview_delete, DeleteSelection};
use crate::device_state::{LocalEntryState, Mapping, RootMarkerId};
use crate::entry_paths::entry_path;
use crate::fetch::{fetch_folders, FetchOutcome, FetchRequest, KeptParcels};
use crate::fetch_conformance::fixtures::exists;
use crate::relocate::Relocation;
use crate::rename::{rename_entries, RenameRequest};
use crate::sync::{sync_folders, Disposal, LibraryKeys, Settled};
use crate::sync_conformance::completion::interrupted_refresh;
use crate::sync_conformance::fixtures::{
    at, keys, map, observed, pending, policy, request, spooled,
};
use crate::sync_conformance::interruption::interrupted;
use crate::sync_conformance::sync_under_test::SyncUnderTest;

// What a catch-up outside a sync does with a pending row of this device's: a
// Container the replay shows current is completed there and then, and nothing
// else is touched (spec: OC-2, OC-7).

/// The catch-up a process makes on its own, as a server's at its start.
async fn catch_up(fixture: &SyncUnderTest, keys: &LibraryKeys, run: i64) {
    catch_up_catalog(CatchUpRequest {
        policy: policy(),
        ..CatchUpRequest::new(
            fixture.store(),
            fixture.index(),
            keys,
            fixture.fs(),
            at(run),
        )
    })
    .await
    .unwrap_or_else(|error| panic!("catching the catalog up must succeed: {error}"));
}

/// A folder fetch over the case's folder.
async fn fetch(fixture: &SyncUnderTest, keys: &LibraryKeys, run: i64) -> FetchOutcome {
    let fs = fixture.fs();
    fetch_folders(
        FetchRequest::new(
            fixture.store(),
            fixture.index(),
            keys,
            fs,
            fs,
            fs,
            fs,
            fs,
            // A folder fetch reads whole objects and keeps no parcel, so the
            // directory is named and never made (spec: PK-16).
            KeptParcels::new(fs, Path::new("/device-state/parcels")),
            at(run),
        )
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("a fetch must succeed: {error}"))
}

/// Whether the case's one file is recorded as this device's.
async fn recorded_present(fixture: &SyncUnderTest) -> bool {
    recorded_present_at(fixture, "a.jpg").await
}

/// Whether the file at `path` is recorded as this device's.
async fn recorded_present_at(fixture: &SyncUnderTest, path: &str) -> bool {
    fixture
        .index()
        .local_entry_at(&entry_path(path))
        .await
        .expect("asking the Index about a local file must succeed")
        .is_some_and(|local| local.state == LocalEntryState::Present)
}

/// A batch whose record landed and whose refresh did not is completed by the
/// next catch-up, before any sync.
///
/// This is a server killed between the commit and the refresh and started
/// again: its start catches the catalog up, and from then on the file the batch
/// carried in is this device's — `present` to a listing, and nothing a fetch
/// declines — rather than another's file standing where its Entry belongs until
/// somebody thinks to sync (spec: OC-7, EP-10).
pub async fn a_catch_up_completes_a_commit_whose_refresh_failed(fixture: &SyncUnderTest) {
    let keys = keys();
    let (landed, path) = interrupted_refresh(fixture, &keys).await;

    catch_up(fixture, &keys, 2).await;

    assert!(
        pending(fixture.index()).await.is_empty(),
        "the row goes with the completion (spec: OC-7)",
    );
    assert_eq!(spooled(fixture.fs()), 0, "and its spool with it");
    let local = fixture
        .index()
        .local_entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index about a local file must succeed")
        .expect("the file the batch carried in is recorded as this device's (spec: EP-10)");
    assert_eq!(local.state, LocalEntryState::Present);
    let (size, mtime) = observed(fixture.fs(), &path);
    assert_eq!(local.observation.size, size);
    assert_eq!(local.observation.mtime, mtime);
    assert!(
        Library::read(fixture.store()).await.holds_container(landed),
        "the Container's object is the Library's and is left where it is",
    );

    let fetched = fetch(fixture, &keys, 3).await;
    assert!(
        fetched.surfaced.is_empty(),
        "the file is this device's own, not one a fetch has to decline: {:?}",
        fetched.surfaced,
    );
    assert!(
        fetched.fetched.is_empty(),
        "and it is already there, so nothing is fetched for it",
    );
    assert!(recorded_present(fixture).await);
}

/// A folder fetch's own catch-up completes the batch before the fetch selects
/// anything.
///
/// The fetch that would otherwise decline every file the batch carried in, as
/// one this device did not put there (spec: EP-10), finds them its own.
pub async fn a_fetch_completes_a_commit_whose_refresh_failed_before_it_selects(
    fixture: &SyncUnderTest,
) {
    let keys = keys();
    interrupted_refresh(fixture, &keys).await;

    let fetched = fetch(fixture, &keys, 2).await;

    assert!(
        fetched.surfaced.is_empty(),
        "no file the batch carried in is taken for another's: {:?}",
        fetched.surfaced,
    );
    assert!(fetched.fetched.is_empty());
    assert!(recorded_present(fixture).await);
    assert!(pending(fixture.index()).await.is_empty());
    assert_eq!(spooled(fixture.fs()), 0);
}

/// A row whose Container is not current is not the catch-up's to touch.
///
/// Uploaded and never committed: whether it may be reclaimed is a question the
/// commit slots its batch recorded answer, which only a sync's settlement reads
/// (spec: OC-2, OC-3). So the catch-up leaves the row, its spool and its object
/// exactly as they were, and the next sync settles them.
pub async fn a_catch_up_leaves_a_row_whose_container_is_not_current(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;
    let abandoned = interrupted(fixture, index, Some(store)).await;
    let before = pending(index).await;

    catch_up(fixture, &keys, 2).await;

    assert_eq!(pending(index).await, before, "the row is left as it was");
    assert_eq!(spooled(fixture.fs()), 1, "and its spool");
    assert!(
        Library::read(store).await.holds_container(abandoned),
        "and its object",
    );

    let outcome = sync_folders(request(store, index, &keys, fixture.fs(), 3))
        .await
        .expect("a sync after the catch-up must succeed");
    assert!(
        matches!(
            &outcome.settled[..],
            [Settled::Disposed {
                container_id,
                disposal: Disposal::Trashed,
            }] if *container_id == abandoned
        ),
        "the sync settles what the catch-up left: {:?}",
        outcome.settled,
    );
}

/// A catch-up while another run owns the pending work leaves the rows to it.
///
/// A sync or a freeze owns this device's pending work for its whole run, and a
/// competing run may not so much as read the rows meanwhile (spec: OC-2). The
/// catch-up itself still succeeds — the catalog is caught up — and the
/// completion waits for a catch-up that finds the rows free.
pub async fn a_catch_up_leaves_rows_another_run_owns(fixture: &SyncUnderTest) {
    let index = fixture.index();
    let keys = keys();
    interrupted_refresh(fixture, &keys).await;

    let owner = index
        .own_pending_rows()
        .await
        .expect("the other run owns the pending work");
    catch_up(fixture, &keys, 2).await;
    assert_eq!(pending(index).await.len(), 1, "the row is the owner's");
    assert_eq!(spooled(fixture.fs()), 1);
    assert!(
        index
            .entry_at(&entry_path("a.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "the catalog is caught up all the same",
    );
    assert!(!recorded_present(fixture).await);

    drop(owner);
    catch_up(fixture, &keys, 3).await;
    assert!(pending(index).await.is_empty());
    assert_eq!(spooled(fixture.fs()), 0);
    assert!(recorded_present(fixture).await);
}

/// A rename's own catch-up completes the batch before the rename plans, so the
/// file the batch carried in follows its Entry.
///
/// Without a materialization record the relocation after the commit has
/// nothing to follow, and the file would stay at the path the batch brought it
/// in at — to be recorded there by a later completion, under a path its Entry
/// no longer has (spec: OC-7, EP-10).
pub async fn a_rename_completes_a_commit_whose_refresh_failed_before_it_plans(
    fixture: &SyncUnderTest,
) {
    let keys = keys();
    interrupted_refresh(fixture, &keys).await;
    let fs = fixture.fs();
    // The folder registered as recording a mapping registers it, so that
    // relocating within it is a write the rename may make (spec: EP-13).
    let registered = RootMarkerId::from_bytes([0x0b, 0x0a, 0x0c, 0x0c, 0x0e, 0x0d, 0x0a, 0x0f]);
    fs.plant_marker(fixture.folder(), &registered);
    fixture
        .index()
        .set_mapping(Mapping::new(None, fixture.folder().to_path_buf()).expecting(registered))
        .await
        .expect("recording a mapping must succeed");

    let outcome = rename_entries(
        RenameRequest::new(
            fixture.store(),
            fixture.index(),
            &keys,
            fs,
            entry_path("a.jpg"),
            entry_path("b.jpg"),
            at(2),
        )
        .with_policy(policy())
        .with_local_disk(fs, fs, fs),
    )
    .await
    .unwrap_or_else(|error| panic!("the rename must commit: {error}"));

    assert!(
        matches!(
            &outcome.relocations[..],
            [Relocation::Relocated { from, to }]
                if *from == entry_path("a.jpg") && *to == entry_path("b.jpg")
        ),
        "the batch's file follows its Entry: {:?}",
        outcome.relocations,
    );
    assert!(!exists(fs, &fixture.folder().join("a.jpg")));
    assert!(exists(fs, &fixture.folder().join("b.jpg")));
    assert!(recorded_present_at(fixture, "b.jpg").await);
    assert!(
        fixture
            .index()
            .local_entry_at(&entry_path("a.jpg"))
            .await
            .expect("asking the Index about a local file must succeed")
            .is_none(),
        "no record is left at the path the Entry no longer has",
    );
    assert!(pending(fixture.index()).await.is_empty());
    assert_eq!(spooled(fs), 0);
}

/// The step before a deletion's preview completes the batch, so the preview is
/// worked out over a catalog that holds the batch's files as this device's
/// local copies, as the deletion's own run would see them (spec: OC-7, PK-9).
pub async fn a_delete_preview_completes_a_commit_whose_refresh_failed(fixture: &SyncUnderTest) {
    let keys = keys();
    interrupted_refresh(fixture, &keys).await;

    let lost = committed_key_lost(
        fixture.store(),
        fixture.index(),
        &keys,
        fixture.fs(),
        &policy(),
        at(2),
    )
    .await
    .unwrap_or_else(|error| panic!("the committed Keyring must be read: {error}"));
    assert!(lost.is_empty());

    assert!(
        recorded_present(fixture).await,
        "the batch's file is this device's local copy (spec: EP-10)",
    );
    assert!(pending(fixture.index()).await.is_empty());
    assert_eq!(spooled(fixture.fs()), 0);

    let preview = preview_delete(
        fixture.index(),
        &DeleteSelection::paths([entry_path("a.jpg")].into()),
        &lost,
    )
    .await
    .unwrap_or_else(|error| panic!("a preview reads the catalog: {error}"));
    assert_eq!(preview.entries, 1);
    assert!(preview.missing.is_empty());
}
