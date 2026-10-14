use coffret_model::{ContainerKind, Generation};

use crate::commit::{commit_batch, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, prepared, request_under, triplicate,
};
use crate::commit_conformance::gathering_store::{Gathered, GatheringStore};
use crate::commit_conformance::library::Library;

/// A commit sends its Keyring replicas to Storage together, and reads them back
/// together (spec: CP-8, KL-2, KL-14).
///
/// No rule orders the replicas: a set is complete by which positions are
/// present, and a replica's name fixes its content. So a commit that waited for
/// one before sending the next would spend a round trip per replica for
/// nothing — on a provider with a slow write, most of a commit. The store holds
/// each replica request until all three are in flight, which only a commit that
/// sends them together gets past without waiting; and the commit still lands,
/// because the head goes out once every replica has been read back.
pub async fn a_commit_writes_and_reads_back_its_replicas_at_once(fixture: &CommitUnderTest) {
    let keys = control_keys();
    Library::upload_container(fixture.store(), container_id(1)).await;

    let gathering = GatheringStore::around(fixture.store(), 3);
    let batch = PreparedBatch::adding(vec![prepared(1, ContainerKind::OneFile, &["albums/a.jpg"])]);
    let outcome = commit_batch(request_under(
        &gathering,
        fixture.index(),
        &keys,
        batch,
        triplicate(),
    ))
    .await
    .expect("a commit into an empty Library must succeed");
    assert_eq!(outcome.record.generation(), Generation::FIRST);

    let together = Gathered {
        arrived: 3,
        waited_alone: false,
    };
    assert_eq!(
        gathering.puts(),
        together,
        "the three replica writes are in flight at once",
    );
    assert_eq!(
        gathering.gets(),
        together,
        "the three read-backs are in flight at once",
    );
}
