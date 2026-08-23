use coffret_model::ContainerKind;

use crate::committed_batch::CommittedBatch;
use crate::device_state::LocalEntryState;
use crate::index_conformance::fixtures::{addition, library_state, observation, pending, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

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
                observation("albums/a.jpg", 100),
                observation("albums/b.jpg", 101),
            ],
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
        .record_pending_upload(pending(1, "batch-alpha"))
        .await
        .expect("recording a spool must succeed");
    let other_batch = pending(9, "batch-beta");
    index
        .record_pending_upload(other_batch.clone())
        .await
        .expect("recording a second spool must succeed");

    index
        .refresh(CommittedBatch {
            record: record(
                0,
                vec![addition(1, ContainerKind::Pack, &["albums/a.jpg"])],
                vec![],
            ),
            materialized: vec![observation("albums/a.jpg", 100)],
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
        .pending_uploads()
        .await
        .expect("reading the spools must succeed");
    assert_eq!(
        pending_now,
        [other_batch],
        "the committed batch's spool is cleared and another batch's is not"
    );
}
