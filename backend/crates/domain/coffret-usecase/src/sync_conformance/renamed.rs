//! What a sync does with the file of an Entry another device renamed
//! (spec: FM-15, EP-10).
//!
//! A rename changes an Entry's current path in the catalog, and the file this
//! device placed for it stays where it was until it is relocated to follow.
//! Until then the file is neither a deletion — its Entry is still in the
//! Library — nor a new file to carry in, nor a departed copy to discard: the
//! sync leaves it exactly where it is and says nothing of it.

use coffret_format::Purpose;
use coffret_model::EntryRename;

use crate::commit_conformance::commit_renames_in;
use crate::device_state::{Mapping, RootMarkerId};
use crate::entry_paths::entry_path;
use crate::sync::{sync_folders, SyncOutcome};
use crate::sync_conformance::fixtures::{keys, purpose_key, request, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// The identity the case registers its folder's root under (spec: EP-13), so
/// that a discard would be one the run could carry out: a case about what is
/// *not* discarded proves nothing through a root nothing may be discarded
/// through.
fn registered() -> RootMarkerId {
    RootMarkerId::from_bytes([0x0e, 0x0a, 0x0e, 0x0d, 0x15, 0x0e, 0x01, 0x05])
}

/// One sync run, which the case expects to succeed.
async fn sync(fixture: &SyncUnderTest, run: i64) -> SyncOutcome {
    let keys = keys();
    sync_folders(request(
        fixture.store(),
        fixture.index(),
        &keys,
        fixture.fs(),
        run,
    ))
    .await
    .unwrap_or_else(|error| panic!("sync run {run} must succeed: {error}"))
}

/// Commits a record renaming `renames` on top of the head this device's
/// catalog stands at, the way another device's rename would land.
async fn commit_renames(fixture: &SyncUnderTest, renames: Vec<EntryRename>) {
    commit_renames_in(
        fixture.store(),
        fixture.index(),
        &purpose_key(Purpose::ControlJournal),
        renames,
    )
    .await;
}

/// The file of an Entry another device renamed is left where it is: the next
/// sync catches the rename up, reports no deletion, carries nothing in, and
/// discards nothing, and the row of the file says which path it waits to
/// follow its Entry to (spec: FM-15, EP-10, EP-15).
pub async fn a_file_whose_entry_was_renamed_is_neither_deleted_nor_added(fixture: &SyncUnderTest) {
    fixture.fs().plant_marker(fixture.folder(), &registered());
    fixture
        .index()
        .set_mapping(Mapping::new(None, fixture.folder().to_path_buf()).expecting(registered()))
        .await
        .expect("recording a mapping must succeed");
    let file = write(
        fixture.fs(),
        fixture.folder(),
        "inbox/a.jpg",
        b"the file's bytes",
    );
    let first = sync(fixture, 1).await;
    assert_eq!(first.added.len(), 1, "the file is carried in");

    let entry = fixture
        .index()
        .entry_at(&entry_path("inbox/a.jpg"))
        .await
        .expect("looking a path up must succeed")
        .expect("the first sync committed the file");
    commit_renames(
        fixture,
        vec![EntryRename::new(
            entry.container_id,
            entry.entry_number,
            entry_path("albums/a.jpg"),
        )
        .expect("an ordinary path is a rename's target")],
    )
    .await;

    for run in [2, 3] {
        let outcome = sync(fixture, run).await;
        assert!(outcome.added.is_empty(), "run {run} carries nothing in");
        assert!(outcome.replaced.is_empty(), "and replaces nothing");
        assert!(outcome.commit.is_none(), "and commits nothing");
        assert!(
            outcome.surfaced.is_empty(),
            "and reports neither a deletion nor a departed file: {:?}",
            outcome.surfaced,
        );
        assert!(
            fixture.fs().discarded().is_empty(),
            "nothing went to the trash"
        );
        assert!(
            fixture.fs().holds(&file),
            "the file stands where it was placed"
        );
    }

    let index = fixture.index();
    assert!(
        index
            .entry_at(&entry_path("inbox/a.jpg"))
            .await
            .expect("looking a path up must succeed")
            .is_none(),
        "the catch-up applied the rename"
    );
    let renamed = index
        .entry_at(&entry_path("albums/a.jpg"))
        .await
        .expect("looking a path up must succeed")
        .expect("the Entry stands at its new path");
    assert_eq!(
        (renamed.container_id, renamed.entry_number),
        (entry.container_id, entry.entry_number)
    );
    assert_eq!(
        index
            .local_entry_at(&entry_path("inbox/a.jpg"))
            .await
            .expect("reading a local file's row must succeed")
            .expect("the row stays with the file")
            .relocate_to,
        Some(entry_path("albums/a.jpg")),
        "and says where the file's Entry stands now"
    );
    assert!(
        index
            .local_entry_at(&entry_path("albums/a.jpg"))
            .await
            .expect("reading a local file's row must succeed")
            .is_none(),
        "no file stands at the new path yet, and none is claimed there"
    );
}
