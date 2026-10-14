//! Settling a batch whose commit was attempted, by the slots it recorded
//! (spec: OC-2, OC-3, OC-7).
//!
//! Every case starts from a Library that has committed once, so that the
//! attempt it interrupts aims at a slot a head carries: on a Storage that mints
//! identifiers, the slot of a Library's first record is one writer's own
//! reservation, and no other writer can ever take it.

use coffret_model::{ContainerKind, Mtime};

use super::fixtures::{keys, map, pending, plant, policy, request, spooled, write};
use super::sync_under_test::SyncUnderTest;
use super::unanswered::{Answer, Unanswered};
use crate::commit::{commit_batch, CommitRequest, PreparedBatch};
use crate::conformance_library::Library;
use crate::device_state::{AttemptedSlot, PendingRow};
use crate::generations::generation;
use crate::in_memory_index::InMemoryIndex;
use crate::index::Index;
use crate::sync::{sync_folders, Disposal, LibraryKeys, Settled};

/// An attempt whose only recorded slot holds another writer's record, which
/// names nothing of the batch, is proven not to have committed and is disposed
/// of like one never attempted: its object trashed, its spool and its row gone
/// (spec: OC-3, OC-6).
pub async fn an_attempt_whose_slot_another_writer_took_is_disposed(fixture: &SyncUnderTest) {
    let keys = keys();
    let index = fixture.index();
    committed_once(fixture, &keys).await;

    let (row, slots) = an_unanswered_attempt(fixture, &keys, Answer::Lost, None).await;
    assert_eq!(
        slots
            .iter()
            .map(AttemptedSlot::generation)
            .collect::<Vec<_>>(),
        [generation(1)],
        "the attempt recorded the one slot it aimed at",
    );

    let rival = InMemoryIndex::new();
    another_writer_commits(fixture, &rival, &keys, "rival.jpg").await;

    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.fs(), 2))
        .await
        .expect("a run whose pending row is proven abandoned succeeds");
    assert!(
        matches!(
            &outcome.settled[..],
            [Settled::Disposed {
                container_id,
                disposal: Disposal::Trashed,
            }] if *container_id == row.container_id
        ),
        "the row is disposed of: {:?}",
        outcome.settled,
    );
    assert!(pending(index).await.is_empty());
    assert_eq!(spooled(fixture.fs()), 0);
    assert!(!Library::read(fixture.store())
        .await
        .holds_container(row.container_id));
    assert!(
        slots_of(index, &row).await.is_empty(),
        "the slots went with the batch's last row",
    );
}

/// A batch that lost the slot and rebased recorded both slots it attempted
/// (spec: OC-2, CP-4), and is disposed of only once both hold another record:
/// while the second is empty the create may still be there to land, and the row
/// is retained (spec: OC-3).
pub async fn a_rebased_attempt_is_disposed_only_once_every_slot_is_proven(fixture: &SyncUnderTest) {
    let keys = keys();
    let index = fixture.index();
    committed_once(fixture, &keys).await;

    // The rival takes the first slot just before this device's create reaches
    // it, so the attempt loses, rebases onto the rival's head, and aims again.
    let rival = InMemoryIndex::new();
    let (row, slots) =
        an_unanswered_attempt(fixture, &keys, Answer::Lost, Some((&rival, "rival.jpg"))).await;
    assert_eq!(
        slots
            .iter()
            .map(AttemptedSlot::generation)
            .collect::<Vec<_>>(),
        [generation(1), generation(2)],
        "the rebased batch recorded both slots it attempted",
    );

    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.fs(), 2))
        .await
        .expect("a run with an uncertain row succeeds");
    assert!(
        matches!(
            &outcome.settled[..],
            [Settled::Retained { container_id }] if *container_id == row.container_id
        ),
        "one slot is still empty, so nothing is proven: {:?}",
        outcome.settled,
    );
    assert_eq!(pending(index).await, std::slice::from_ref(&row));
    assert_eq!(spooled(fixture.fs()), 1);
    assert!(Library::read(fixture.store())
        .await
        .holds_container(row.container_id));

    another_writer_commits(fixture, &rival, &keys, "rival-2.jpg").await;

    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.fs(), 3))
        .await
        .expect("a run whose pending row is proven abandoned succeeds");
    assert!(
        matches!(
            &outcome.settled[..],
            [Settled::Disposed {
                container_id,
                disposal: Disposal::Trashed,
            }] if *container_id == row.container_id
        ),
        "every slot is taken by another record now: {:?}",
        outcome.settled,
    );
    assert!(pending(index).await.is_empty());
    assert_eq!(spooled(fixture.fs()), 0);
    assert!(!Library::read(fixture.store())
        .await
        .holds_container(row.container_id));
}

/// A record at an attempted slot that names the row's Container is the batch's
/// own commit, whose answer never arrived, and the row is completed rather than
/// reclaimed — even once a later commit has removed the Container, so that the
/// current set no longer says it landed (spec: OC-3, OC-7).
pub async fn a_record_at_an_attempted_slot_naming_the_container_completes_it(
    fixture: &SyncUnderTest,
) {
    let keys = keys();
    let index = fixture.index();
    committed_once(fixture, &keys).await;

    let (row, _) = an_unanswered_attempt(fixture, &keys, Answer::LostAfterLanding, None).await;

    // Another writer removes the Container this device's record added, so
    // nothing but the slot says the batch committed.
    let rival = InMemoryIndex::new();
    commit_batch(
        CommitRequest::new(
            fixture.store(),
            &rival,
            keys.control(),
            PreparedBatch::default().removing(vec![row.container_id]),
        )
        .with_policy(policy()),
    )
    .await
    .expect("another writer's removal commits");

    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.fs(), 2))
        .await
        .expect("a run whose pending row's batch committed succeeds");
    assert!(
        matches!(
            &outcome.settled[..],
            [Settled::Completed { container_id, entries: 0 }] if *container_id == row.container_id
        ),
        "the row is completed: {:?}",
        outcome.settled,
    );
    assert!(pending(index).await.is_empty());
    assert_eq!(spooled(fixture.fs()), 0);
}

/// An attempted slot that is empty proves nothing, and neither does one whose
/// record has gone from the Library — pruned, or deleted with its epoch — even
/// where the provider would still hand its bytes back out of a trash. The row is
/// retained either way (spec: OC-3).
pub async fn an_empty_or_vanished_slot_proves_nothing(fixture: &SyncUnderTest) {
    let keys = keys();
    let index = fixture.index();
    committed_once(fixture, &keys).await;

    let (row, slots) = an_unanswered_attempt(fixture, &keys, Answer::Lost, None).await;

    let retained = |outcome: &[Settled]| {
        matches!(
            outcome,
            [Settled::Retained { container_id }] if *container_id == row.container_id
        )
    };
    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.fs(), 2))
        .await
        .expect("a run with an uncertain row succeeds");
    assert!(
        retained(&outcome.settled),
        "an empty slot proves nothing: {:?}",
        outcome.settled,
    );

    let rival = InMemoryIndex::new();
    another_writer_commits(fixture, &rival, &keys, "rival.jpg").await;
    let [slot] = slots.as_slice() else {
        panic!("the attempt recorded one slot, not {slots:?}");
    };
    let record = fixture
        .store()
        .object_at(&slot.commit_slot())
        .expect("the slot names its object");
    fixture
        .store()
        .trash(&record)
        .await
        .expect("the record at the slot goes");

    let outcome = sync_folders(request(fixture.store(), index, &keys, fixture.fs(), 3))
        .await
        .expect("a run with an uncertain row succeeds");
    assert!(
        retained(&outcome.settled),
        "a record that is gone proves nothing: {:?}",
        outcome.settled,
    );
    assert_eq!(pending(index).await, std::slice::from_ref(&row));
    assert_eq!(spooled(fixture.fs()), 1);
    assert!(Library::read(fixture.store())
        .await
        .holds_container(row.container_id));
}

/// Commits one Container from this device, so the Library has a head whose
/// carried slot the next attempt aims at.
async fn committed_once(fixture: &SyncUnderTest, keys: &LibraryKeys) {
    map(fixture, None).await;
    plant(
        fixture.store(),
        fixture.index(),
        keys,
        ContainerKind::OneFile,
        "base.jpg",
        b"the Library's first file",
        Mtime::from_unix_seconds(1_700_000_000),
        false,
    )
    .await;
}

/// Commits one Container from another writer's catalog.
async fn another_writer_commits(
    fixture: &SyncUnderTest,
    rival: &dyn Index,
    keys: &LibraryKeys,
    path: &str,
) {
    plant(
        fixture.store(),
        rival,
        keys,
        ContainerKind::OneFile,
        path,
        b"another writer's file",
        Mtime::from_unix_seconds(1_700_000_000),
        false,
    )
    .await;
}

/// Runs a sync of one new file whose Journal create never answers, and hands
/// back the row it left and the slots its batch recorded.
///
/// The file is removed once the run has failed, so that the next run has
/// nothing to carry in and settles the row alone.
async fn an_unanswered_attempt(
    fixture: &SyncUnderTest,
    keys: &LibraryKeys,
    answer: Answer,
    rival: Option<(&dyn Index, &'static str)>,
) -> (PendingRow, Vec<AttemptedSlot>) {
    let index = fixture.index();
    let file = write(
        fixture.fs(),
        fixture.folder(),
        "a.jpg",
        b"this device's file",
    );
    let store = Unanswered::around(
        fixture.store(),
        answer,
        rival.map(|(index, path)| (index, path, keys)),
    );
    let failed = sync_folders(request(&store, index, keys, fixture.fs(), 1)).await;
    assert!(failed.is_err(), "the create never answered: {failed:?}");
    fixture.fs().remove_file(&file);

    let mut rows = pending(index).await;
    assert_eq!(rows.len(), 1, "one Container was spooled: {rows:?}");
    let row = rows.remove(0);
    assert!(row.commit_attempted, "the mark precedes the create");
    let slots = slots_of(index, &row).await;
    assert!(!slots.is_empty(), "the slot is recorded with the mark");
    (row, slots)
}

async fn slots_of(index: &dyn Index, row: &PendingRow) -> Vec<AttemptedSlot> {
    index
        .attempted_slots(&row.batch)
        .await
        .expect("reading the attempted slots must succeed")
}
