use coffret_format::{unwrap_container_key, Purpose};
use coffret_model::{
    ContainerKey, ContainerKeyStatus, ContainerKind, ControlObjectName, Generation,
    KeyringCommitment,
};

use crate::commit::{commit_batch, CommitError, PreparedBatch};
use crate::commit_conformance::commit_under_test::CommitUnderTest;
use crate::commit_conformance::faulty_store::FaultyStore;
use crate::commit_conformance::fixtures::{
    container_id, control_keys, prepared, purpose_key, request,
};
use crate::commit_conformance::library::{mapped, Library};
use crate::error::Error;
use crate::generations::generation;

/// CP-9: the previously committed Keyring remains authoritative until the
/// Journal commit, so the candidate excludes removed Containers without making
/// the pre-commit state unreadable.
///
/// The batch removes a Container and its record is refused, so the candidate
/// that leaves the Container out is on Storage and selects nothing. The
/// Container is still current, and a device reading the Library still opens
/// it: the committed generation maps it to an envelope that unwraps to its
/// key, and its object is where it was, trashing coming only after a commit
/// (spec: OC-6). The candidate, read back under the tuple its names carry,
/// maps exactly the post-commit set and not the Container being removed
/// (spec: CP-8).
pub async fn a_candidate_without_a_removal_leaves_the_committed_keyring_readable(
    fixture: &CommitUnderTest,
) {
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
    let committed = commit_batch(request(store, index, &keys, first))
        .await
        .expect("the first commit must succeed")
        .record
        .keyring()
        .clone();

    let second =
        PreparedBatch::adding(vec![prepared(3, ContainerKind::OneFile, &["albums/a.jpg"])])
            .removing(vec![container_id(1)]);
    let refusing = FaultyStore::refusing_the_head(store);
    let result = commit_batch(request(&refusing, index, &keys, second))
        .await
        .map_err(|failure| *failure.error);
    assert!(
        matches!(result, Err(CommitError::Storage(Error::Rejected { .. }))),
        "expected the create of the record to be refused, got {result:?}",
    );

    let library = Library::read(store).await;
    assert!(
        !library.holds(&ControlObjectName::head(generation(2))),
        "the batch never reached its Journal commit",
    );
    assert_eq!(
        library.record(store, generation(1)).await.keyring(),
        &committed,
        "the head still names the generation committed before the batch",
    );

    let elements = library.keyring(store, &committed).await;
    assert_eq!(
        mapped(&elements),
        vec![container_id(1), container_id(2)],
        "the committed generation still maps the Container the batch removes",
    );
    let removed = elements
        .iter()
        .find(|element| element.container_id == container_id(1))
        .expect("the committed generation maps the Container");
    let ContainerKeyStatus::Envelope(envelope) = &removed.key else {
        panic!("the committed generation holds an envelope for the Container");
    };
    let key = unwrap_container_key(
        &purpose_key(Purpose::ContainerWrap),
        &container_id(1),
        envelope,
    )
    .expect("the committed envelope still opens");
    assert_eq!(
        key.as_bytes(),
        &[1; ContainerKey::BYTE_LEN],
        "and yields the Container's own key",
    );
    assert!(
        library.holds_container(container_id(1)),
        "the Container's object is untouched until a commit removes it",
    );

    let candidate = candidate_at(&library, generation(2));
    assert_eq!(
        mapped(&library.keyring(store, &candidate).await),
        vec![container_id(2), container_id(3)],
        "the candidate maps (current - removals) union additions and not the removal",
    );
}

/// The commitment one generation's replicas on Storage carry in their names,
/// for a candidate no record names (spec: FM-12).
fn candidate_at(library: &Library, wanted: Generation) -> KeyringCommitment {
    let mut found = library
        .names()
        .into_iter()
        .filter_map(|name| match ControlObjectName::parse(&name) {
            Ok(ControlObjectName::KeyringReplica {
                generation,
                set_digest,
                replica,
            }) if generation == wanted => Some((set_digest, replica.count())),
            _ => None,
        })
        .collect::<Vec<_>>();
    found.dedup();
    let [(set_digest, replica_count)] = &found[..] else {
        panic!("expected one replica set at generation {wanted:?}, got {found:?}");
    };
    KeyringCommitment::new(wanted, *replica_count, set_digest)
        .expect("a replica name carries a valid commitment")
}
