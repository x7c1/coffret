use coffret_model::ObjectRef;

use crate::device_state::{PendingRow, SpoolState};
use crate::index_conformance::fixtures::{container_id, pending, spooling};
use crate::index_conformance::index_under_test::IndexUnderTest;
use crate::index_error::IndexError;

/// A Spooling row becomes Spooled when its spool file does, and only then
/// (spec: OC-2).
///
/// The row is written before the file it names exists, so what the catalog has
/// to carry is the difference between a spool file this device announced and one
/// it finished — the first being ciphertext worth nothing to anybody and the
/// second the only kind that is ever uploaded or committed.
///
/// Marking is a narrow flip and not an upsert: everything the announcing row
/// said about the Container is left alone, because none of it changed when the
/// file did. Marking an already-Spooled row changes nothing, so an interrupted
/// spool step is simply run again. And marking a Container the catalog holds
/// no row for changes nothing either, rather than failing or inventing one —
/// a row is what a spool step announced, and the operation says that such a
/// row's file is whole.
pub async fn a_spooling_row_becomes_spooled_when_its_file_completes(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .record_pending_row(spooling(1, "batch-alpha"))
        .await
        .expect("recording a Spooling row must succeed");
    assert_eq!(
        index
            .pending_rows()
            .await
            .expect("reading the spools must succeed"),
        [spooling(1, "batch-alpha")],
        "a row read back is the row that was written, its state included",
    );

    index
        .mark_spooled(container_id(1))
        .await
        .expect("marking the Container spooled must succeed");
    assert_eq!(
        index
            .pending_rows()
            .await
            .expect("reading the spools must succeed"),
        [PendingRow {
            commit_attempted: false,
            state: SpoolState::Spooled(None),
            ..spooling(1, "batch-alpha")
        }],
        "the state moves and nothing else does",
    );

    index
        .mark_spooled(container_id(1))
        .await
        .expect("marking an already-Spooled row must succeed");
    index
        .mark_spooled(container_id(9))
        .await
        .expect("marking a Container with no row must succeed");
    assert_eq!(
        index
            .pending_rows()
            .await
            .expect("reading the spools must succeed"),
        [PendingRow {
            commit_attempted: false,
            state: SpoolState::Spooled(None),
            ..spooling(1, "batch-alpha")
        }],
        "neither repeating the flip nor marking an unannounced spool changes the catalog",
    );

    // An uploaded row is already whole, and marking it again leaves the handle
    // it names where it was.
    index
        .record_pending_row(pending(2, "batch-alpha"))
        .await
        .expect("recording an uploaded row must succeed");
    index
        .mark_spooled(container_id(2))
        .await
        .expect("marking an uploaded row must succeed");
    let rows = index
        .pending_rows()
        .await
        .expect("reading the spools must succeed");
    assert!(
        rows.contains(&pending(2, "batch-alpha")),
        "marking an uploaded row must keep the object it names, got {rows:?}",
    );
}

/// A row still spooling that names an uploaded object is refused when read
/// (spec: OC-2).
///
/// A Container is uploaded only out of a finished spool, and the port has no
/// way to say otherwise: [`SpoolState::Spooling`] carries no object handle. A
/// stored form that keeps the state and the handle apart can still hold the
/// pair, written by something other than this build, and reading it back as
/// either state would be a guess — a spool to reclaim locally, or an object on
/// Storage to dispose of. So the catalog refuses it as one it cannot read.
///
/// An implementation whose stored form is the row itself holds the rule by
/// type and hands the suite no
/// [`StoredForm`](crate::index_conformance::StoredForm); the case has nothing
/// to plant there.
pub async fn a_spooling_row_that_names_an_object_is_refused(fixture: &IndexUnderTest) {
    let Some(stored) = fixture.stored_form() else {
        return;
    };
    stored.plant_spooling_row_with_object(&spooling(1, "batch-alpha"), &ObjectRef::new("stored-1"));

    let read = fixture.index().pending_rows().await;

    assert!(
        matches!(read, Err(IndexError::UnreadableCatalog { .. })),
        "a spooling row that names an object must be refused, got {read:?}",
    );
}

/// A spool is recorded until its batch commits or is abandoned, and clearing it
/// twice is not an error (spec: OC-2, OC-8).
///
/// The rows round-trip whole, their states included: what a later run reads is
/// what the run that spooled wrote down.
pub async fn a_spool_is_recorded_until_its_row_is_cleared(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .record_pending_row(pending(1, "batch-alpha"))
        .await
        .expect("recording a spool must succeed");
    index
        .record_pending_row(pending(2, "batch-alpha"))
        .await
        .expect("recording a second spool must succeed");
    assert_eq!(
        index
            .pending_rows()
            .await
            .expect("reading the spools must succeed"),
        [pending(1, "batch-alpha"), pending(2, "batch-alpha")]
    );

    index
        .clear_pending_row(container_id(1))
        .await
        .expect("clearing a spool must succeed");
    index
        .clear_pending_row(container_id(1))
        .await
        .expect("clearing a spool already cleared must succeed");

    assert_eq!(
        index
            .pending_rows()
            .await
            .expect("reading the spools must succeed"),
        [pending(2, "batch-alpha")]
    );
}
