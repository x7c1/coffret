use coffret_model::ContainerKind;

use crate::committed_batch::CommittedBatch;
use crate::device_state::LocalEntryState;
use crate::index_conformance::fixtures::{
    addition, entry, library_state, observation_of, path, pending, record,
};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_error::IndexError;

/// This device's own commit lands the Library-wide state another device's
/// replay of it does.
///
/// A commit changes the current Container set exactly once, at the record
/// (spec: CP-1), and the committer has no privileged view of what it changed.
/// If the two diverged, one device's checkpoint would disagree with another's
/// at the same head — and either could be the one that writes the Snapshot the
/// rest of them adopt (spec: CK-8, CK-11).
pub async fn a_refresh_lands_where_a_replay_of_its_record_does(fixture: &IndexUnderTest) {
    let committed = record(
        0,
        vec![addition(
            1,
            ContainerKind::Pack,
            &["albums/a.jpg", "albums/b.jpg"],
        )],
        vec![],
    );

    fixture
        .index()
        .refresh(CommittedBatch {
            record: committed.clone(),
            materialized: vec![
                observation_of("albums/a.jpg", 100, entry(1, 0)),
                observation_of("albums/b.jpg", 101, entry(1, 1)),
            ],
            released: Vec::new(),
        })
        .await
        .expect("refreshing after a commit must succeed");

    fixture
        .other()
        .apply(committed)
        .await
        .expect("replaying the same record must succeed");

    let committer = fixture
        .index()
        .snapshot()
        .await
        .expect("the committer has a state to checkpoint");
    let replayer = fixture
        .other()
        .snapshot()
        .await
        .expect("the replayer has a state to checkpoint");

    assert_eq!(library_state(committer), library_state(replayer));
}

/// A refresh also records what only the committer knows.
///
/// The files it put on disk become the ones it may later report as deleted
/// locally (spec: EP-10), and the Containers it uploaded stop being pending,
/// because a Container a commit made current is no longer a candidate for
/// orphan cleanup (spec: OC-2). A spool belonging to a different, still
/// uncommitted batch is left exactly where it was.
pub async fn a_refresh_marks_its_files_present_and_clears_its_spools(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .record_pending_row(pending(1, "batch-alpha"))
        .await
        .expect("recording a spool must succeed");
    let other_batch = pending(9, "batch-beta");
    index
        .record_pending_row(other_batch.clone())
        .await
        .expect("recording a second spool must succeed");

    index
        .refresh(CommittedBatch {
            record: record(
                0,
                vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
                vec![],
            ),
            materialized: vec![observation_of("albums/a.jpg", 100, entry(1, 0))],
            released: Vec::new(),
        })
        .await
        .expect("refreshing after a commit must succeed");

    let present = index
        .present_under(None)
        .await
        .expect("reading what this device has must succeed");
    assert_eq!(
        present.len(),
        1,
        "one file was materialized, got {present:?}"
    );
    assert_eq!(present[0].observation.path.as_str(), "albums/a.jpg");
    assert_eq!(present[0].state, LocalEntryState::Present);

    let pending_now = index
        .pending_rows()
        .await
        .expect("reading the spools must succeed");
    assert_eq!(
        pending_now,
        [other_batch],
        "the committed batch's spool is cleared and another batch's is not"
    );
}

/// Where another process applied the record first, completing the batch
/// records exactly the device-local half (spec: CK-9, EP-10, OC-2, PK-17).
///
/// Two processes over one catalog file — a server beside a terminal `sync` —
/// can both reach a fresh record: the committer's refresh is then refused as
/// already applied, with nothing of its own recorded. What is left is what
/// only the committer knows. Its files become `present`, its spool stops being
/// pending and the reservation its batch packs is released, in one operation;
/// another batch's spool and another drop's reservation stay, and the catalog
/// stands where the replay left it.
pub async fn a_refresh_another_process_got_to_first_is_completed(fixture: &IndexUnderTest) {
    let index = fixture.index();
    index
        .record_pending_row(pending(1, "batch-alpha"))
        .await
        .expect("recording a spool must succeed");
    let other_batch = pending(9, "batch-beta");
    index
        .record_pending_row(other_batch.clone())
        .await
        .expect("recording a second spool must succeed");
    let packed = index
        .reserve(Some(&path("albums")))
        .await
        .expect("recording a reservation must succeed");
    let waiting = index
        .reserve(Some(&path("books")))
        .await
        .expect("recording a reservation must succeed");

    let committed = record(
        0,
        vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
        vec![],
    );
    // The other process's replay.
    index
        .apply(committed.clone())
        .await
        .expect("replaying a record must succeed");
    let replayed = index
        .snapshot()
        .await
        .expect("an applied catalog has a state to checkpoint");

    let batch = CommittedBatch {
        record: committed,
        materialized: vec![observation_of("albums/a.jpg", 100, entry(1, 0))],
        released: vec![packed],
    };
    let refused = index.refresh(batch.clone()).await;
    assert!(
        matches!(&refused, Err(IndexError::AlreadyApplied { .. })),
        "expected the refresh to be refused as already applied, got {refused:?}"
    );
    index
        .complete_applied(batch)
        .await
        .expect("completing a batch whose record is applied must succeed");

    let present = index
        .present_under(None)
        .await
        .expect("reading what this device has must succeed");
    assert_eq!(
        present.len(),
        1,
        "one file was materialized, got {present:?}"
    );
    assert_eq!(present[0].observation.path.as_str(), "albums/a.jpg");
    assert_eq!(present[0].state, LocalEntryState::Present);
    assert_eq!(
        index
            .pending_rows()
            .await
            .expect("reading the spools must succeed"),
        [other_batch],
        "the committed batch's spool is cleared and another batch's is not"
    );
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed")
            .into_iter()
            .map(|reservation| reservation.id)
            .collect::<Vec<_>>(),
        [waiting],
        "the batch packed one reservation, and only that one went with it",
    );
    assert_eq!(
        index
            .snapshot()
            .await
            .expect("an applied catalog has a state to checkpoint"),
        replayed,
        "completing the batch leaves the catalog where the replay left it",
    );
}
