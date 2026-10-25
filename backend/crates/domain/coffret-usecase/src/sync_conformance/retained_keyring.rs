use coffret_model::{ControlObjectName, Generation};

use super::fixtures::{keys, map, policy, request, write};
use super::sync_under_test::SyncUnderTest;
use super::withheld_commit::WithheldCommit;
use crate::object_store::ObjectStore;
use crate::sync::{sync_folders, RetainedKeyringReplicas, Settled};

/// A commit interrupted after its Keyring replicas were put leaves a
/// generation no head names, and the next run reports it as retained Keyring
/// replicas — and keeps them, since a suspected orphan is reported for review
/// and never removed (spec: OC-4).
///
/// The head the batch created is withheld from the listings after it lands,
/// so the run after it stands where a device does whose commit never reached
/// its head: the candidate's replicas are on Storage, one past the committed
/// generation, and nothing names them. Once the head is listed again it names
/// them, and nothing is reported: what was suspected was a commit after all.
///
/// What is reported is the number of objects, not of names. Every attempt the
/// interrupted commit made wrote the candidate's replicas again (spec: KL-14),
/// which on a store that keys objects by name leaves one object per position
/// and on one that mints identifiers leaves one per attempt beside the others,
/// all of them on Storage for a person to look at.
pub async fn keyring_replicas_no_head_names_are_reported_and_kept(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;
    write(fixture.fs(), fixture.folder(), "a.jpg", b"committed bytes");
    let withheld = WithheldCommit::around(store);
    let failed = sync_folders(request(&withheld, index, &keys, fixture.fs(), 1)).await;
    assert!(failed.is_err(), "the commit response is unavailable");
    fixture.fs().remove_file(&fixture.folder().join("a.jpg"));

    for run in [2, 3] {
        let outcome = sync_folders(request(&withheld, index, &keys, fixture.fs(), run))
            .await
            .unwrap_or_else(|error| panic!("a run over the older history must succeed: {error}"));
        let committed = index
            .checkpoint()
            .await
            .expect("the catalog says where it stands")
            .expect("the catalog stands at the Library's first head")
            .keyring()
            .generation();
        let candidate = committed.next().expect("a small generation has a next");
        let replicas = replica_objects(&withheld, candidate).await;
        assert!(
            replicas >= usize::from(policy().replica_count),
            "every position of the candidate was put at least once: {replicas}",
        );
        assert_eq!(
            outcome.retained_keyrings,
            [RetainedKeyringReplicas {
                generation: candidate,
                replicas,
            }],
            "the candidate's replicas are reported, one generation past the committed one",
        );
        assert!(
            matches!(&outcome.settled[..], [Settled::Retained { .. }]),
            "beside the Container of the same batch, retained as well",
        );
    }

    let recovered = sync_folders(request(store, index, &keys, fixture.fs(), 4))
        .await
        .unwrap_or_else(|error| panic!("a run over the whole history must succeed: {error}"));
    assert!(
        recovered.retained_keyrings.is_empty(),
        "the head is listed again and names the generation: {:?}",
        recovered.retained_keyrings,
    );
}

/// How many objects Storage lists under a Keyring replica name of one
/// generation, counting each object under a name that carries several.
async fn replica_objects(store: &dyn ObjectStore, generation: Generation) -> usize {
    let mut count = 0;
    let mut token = None;
    loop {
        let page = store
            .list(token.as_ref())
            .await
            .expect("listing a store must succeed");
        count += page
            .objects
            .iter()
            .filter(|object| {
                matches!(
                    ControlObjectName::parse(&object.name),
                    Ok(ControlObjectName::KeyringReplica { generation: listed, .. })
                        if listed == generation
                )
            })
            .count();
        token = page.next;
        if token.is_none() {
            return count;
        }
    }
}
