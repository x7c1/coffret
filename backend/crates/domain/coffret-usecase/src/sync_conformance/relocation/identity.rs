use coffret_format::Purpose;
use coffret_model::EntryRename;

use crate::commit_conformance::commit_renames_in;
use crate::delete::{delete_entries, DeleteRequest, DeleteSelection};
use crate::device_state::BatchId;
use crate::entry_paths::entry_path;
use crate::in_memory_index::InMemoryIndex;
use crate::index::Index;
use crate::relocate::RelocationRefused;
use crate::sync_conformance::fixtures::{at as clock, keys, policy, purpose_key, spool_dir};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

use super::{carry_in, content, refused, rename, row, sync, uploaded_nothing};

/// Deletes the Entry at `path` from another device, whose catalog is
/// `elsewhere`, under `policy`.
async fn delete_elsewhere(
    fixture: &SyncUnderTest,
    elsewhere: &dyn Index,
    path: &str,
    checkpointing: bool,
) {
    let keys = keys();
    let policy = if checkpointing {
        policy().with_checkpoint_threshold(1)
    } else {
        policy()
    };
    let outcome = delete_entries(
        DeleteRequest::new(
            fixture.store(),
            elsewhere,
            &keys,
            fixture.fs(),
            spool_dir(),
            DeleteSelection::paths([entry_path(path)].into()),
            BatchId::new("delete-elsewhere"),
            clock(50),
        )
        .with_policy(policy),
    )
    .await
    .unwrap_or_else(|error| panic!("deleting {path} must succeed: {error}"));
    assert_eq!(outcome.removed.len(), 1, "the Entry's Container is removed");
}

/// Asserts what neither identity case may do: claim the file at `a.jpg` for
/// the Entry now standing there, or offer it as that Entry's update.
async fn a_jpg_is_claimed_for_nothing(fixture: &SyncUnderTest, outcome: &crate::sync::SyncOutcome) {
    uploaded_nothing(outcome);
    assert!(
        outcome.surfaced.is_empty(),
        "no update, deletion or departure is proposed: {:?}",
        outcome.surfaced
    );
    assert!(
        row(fixture, "a.jpg").await.is_none(),
        "the file at a.jpg is not a copy this device placed of the Entry there"
    );
    assert_eq!(content(fixture, "a.jpg").as_deref(), Some(&b"first"[..]));
    assert!(fixture.fs().discarded().is_empty(), "and it is left alone");
    let (from, to, reason) = refused(outcome);
    assert_eq!((from.as_str(), to.as_str()), ("c.jpg", "a.jpg"));
    assert!(
        matches!(reason, RelocationRefused::DestinationTaken),
        "{reason:?}"
    );
    assert_eq!(
        row(fixture, "c.jpg")
            .await
            .expect("the renamed Entry's file keeps its record")
            .relocate_to,
        Some(entry_path("a.jpg"))
    );
}

/// The file of an Entry another device deleted is departed at its path, and a
/// later rename of another Entry onto that path does not make it that Entry's
/// copy: its record is forgotten rather than offered as the newcomer's edit,
/// and the newcomer's own file waits rather than overwrite it (spec: EP-10,
/// EP-11, EP-15).
pub async fn a_departed_file_is_not_claimed_by_an_entry_renamed_onto_its_path(
    fixture: &SyncUnderTest,
) {
    carry_in(fixture, &[("a.jpg", b"first"), ("c.jpg", b"the other one")]).await;
    let elsewhere = InMemoryIndex::new();
    delete_elsewhere(fixture, &elsewhere, "a.jpg", false).await;
    let other = elsewhere
        .entry_at(&entry_path("c.jpg"))
        .await
        .expect("looking a path up must succeed")
        .expect("the other Entry stands");
    commit_renames_in(
        fixture.store(),
        &elsewhere,
        &purpose_key(Purpose::ControlJournal),
        vec![
            EntryRename::new(other.container_id, other.entry_number, entry_path("a.jpg"))
                .expect("an ordinary path is a rename's target"),
        ],
    )
    .await;

    let outcome = sync(fixture, 2).await;

    a_jpg_is_claimed_for_nothing(fixture, &outcome).await;
}

/// A device that adopts a Snapshot past two Entries swapping their names and
/// the removal of one of them claims neither file for the wrong Entry: the
/// removed Entry's file is not taken for the one now at its path, and the other
/// Entry's file waits to follow it there rather than overwrite it
/// (spec: CK-9, EP-10, FM-15).
pub async fn a_snapshot_past_a_swap_and_a_removal_claims_neither_file(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", b"first"), ("c.jpg", b"the other one")]).await;
    rename(fixture, &[("a.jpg", "c.jpg"), ("c.jpg", "a.jpg")]).await;
    // The Entry this device's `a.jpg` is a copy of now stands at `c.jpg`, and
    // another device deletes it there and checkpoints the Library after it.
    let elsewhere = InMemoryIndex::new();
    delete_elsewhere(fixture, &elsewhere, "c.jpg", true).await;

    let outcome = sync(fixture, 2).await;

    let (_, adopted_from, _, _) = fixture
        .index()
        .snapshot()
        .await
        .expect("a caught-up catalog has a state to checkpoint")
        .into_parts();
    assert!(
        adopted_from.is_some(),
        "the catch-up adopted the Snapshot rather than replaying the records"
    );
    a_jpg_is_claimed_for_nothing(fixture, &outcome).await;
}
