use coffret_model::{ContainerKind, Generation};

use crate::commit::{commit_batch, CheckpointOutcome, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::fixtures::{container_id, control_keys, path, prepared, request};
use crate::commit_conformance::library::{mapped, Library};

/// A commit makes the batch the Library's current state (spec: CP-1).
///
/// Four things have to be true together for that claim to mean anything, and
/// only one of them is about the call's return value. The record another device
/// would fetch has to decode to the batch that was committed (spec: FM-15); the
/// Keyring tuple that record names has to be a complete, valid set on Storage
/// (spec: CP-10, KL-1, KL-2); and this device's own Index has to answer the new
/// state without being told again (spec: CP-1, EP-5).
pub async fn a_commit_makes_the_batch_the_current_state(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    Library::upload_container(store, container_id(1)).await;
    Library::upload_container(store, container_id(2)).await;

    let batch = PreparedBatch::adding(vec![
        prepared(1, ContainerKind::OneFile, &["albums/a.jpg"]),
        prepared(2, ContainerKind::Pack, &["books/p-1.png", "books/p-2.png"]),
    ]);
    let outcome = commit_batch(request(store, index, &keys, batch))
        .await
        .expect("a commit into an empty Library must succeed");

    // The Library's first head is generation 0 and succeeds nothing
    // (spec: FM-13).
    assert_eq!(outcome.record.generation, Generation::FIRST);
    assert_eq!(outcome.record.prev, None);
    assert_eq!(outcome.attempts, 1);
    assert!(matches!(outcome.checkpoint, CheckpointOutcome::NotDue));
    assert!(outcome.untrashed.is_empty());

    let library = Library::read(store).await;
    assert_eq!(
        library.record(store, Generation::FIRST).await,
        outcome.record,
        "the record on Storage is the one the commit reported",
    );

    let keyring = library.keyring(store, &outcome.record.keyring).await;
    assert_eq!(
        mapped(&keyring),
        vec![container_id(1), container_id(2)],
        "the committed Keyring maps every current Container and no other (spec: KL-7)",
    );
    assert_eq!(outcome.record.keyring.generation(), Generation::FIRST);

    for (text, held_by) in [
        ("albums/a.jpg", container_id(1)),
        ("books/p-1.png", container_id(2)),
        ("books/p-2.png", container_id(2)),
    ] {
        let location = index
            .entry_at(&path(text))
            .await
            .expect("asking the Index for a path must succeed")
            .expect("the committed Entry is current");
        assert_eq!(location.container_id, held_by);
    }
    let checkpoint = index
        .checkpoint()
        .await
        .expect("reading the checkpoint must succeed")
        .expect("a committed device stands at a committed state");
    assert_eq!(checkpoint.head_generation, Generation::FIRST);
    assert_eq!(checkpoint.keyring, outcome.record.keyring);
}

/// A removal leaves the current set, and its object is trashed.
///
/// The record is what makes the Container non-current (spec: CP-1, CP-14) and
/// trashing the object is what happens after, which is why the two are asserted
/// separately. Trashed and not purged: the object leaves the listing and stays
/// restorable, which is what removing a Container means. The batch also moves
/// an Entry Path from the Container it removes to the one it adds, which is the
/// reordering EP-6 exists for: removals leave the path map before additions
/// enter it.
pub async fn a_removal_leaves_the_current_set_and_is_trashed(fixture: &CommitUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = control_keys();

    for seed in [1, 2, 3] {
        Library::upload_container(store, container_id(seed)).await;
    }
    let first = PreparedBatch::adding(vec![
        prepared(1, ContainerKind::OneFile, &["albums/a.jpg"]),
        prepared(2, ContainerKind::OneFile, &["albums/b.jpg"]),
    ]);
    commit_batch(request(store, index, &keys, first))
        .await
        .expect("the first commit must succeed");

    // Container 3 replaces Container 1 at the same Entry Path.
    let second =
        PreparedBatch::adding(vec![prepared(3, ContainerKind::OneFile, &["albums/a.jpg"])])
            .removing(vec![container_id(1)]);
    let outcome = commit_batch(request(store, index, &keys, second))
        .await
        .expect("replacing a Container must succeed");

    assert_eq!(outcome.record.generation, Generation::new(1));
    assert_eq!(outcome.record.prev, Some(Generation::FIRST));
    assert_eq!(outcome.record.removals, vec![container_id(1)]);
    assert!(
        outcome.untrashed.is_empty(),
        "every removed Container's object was trashed",
    );

    let library = Library::read(store).await;
    assert!(
        !library.holds_container(container_id(1)),
        "a removed Container's object leaves the listing",
    );
    assert!(library.holds_container(container_id(2)));
    assert!(library.holds_container(container_id(3)));

    let keyring = library.keyring(store, &outcome.record.keyring).await;
    assert_eq!(
        mapped(&keyring),
        vec![container_id(2), container_id(3)],
        "the next generation covers (current - removals) union additions (spec: CP-8)",
    );
    assert_eq!(
        outcome.record.keyring.generation(),
        Generation::new(1),
        "each commit prepares a generation of its own (spec: KL-9, KL-10)",
    );

    let moved = index
        .entry_at(&path("albums/a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the path is still current, held by the replacement");
    assert_eq!(moved.container_id, container_id(3));
}
