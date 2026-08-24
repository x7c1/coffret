use std::collections::BTreeMap;

use coffret_model::{
    ContainerId, ControlObjectName, KeyringCommitment, ObjectRef, ReplicaPosition,
};

use crate::byte_stream::ByteStream;
use crate::object_store::ObjectStore;

/// Every object in Storage, by the name it is stored under.
///
/// A store that mints identifiers does not name objects by their names, so a
/// case cannot turn `head-0.cfrt` into something [`ObjectStore::get`] accepts —
/// only the listing can (spec: FM-3, FM-12).
pub(super) async fn handles(store: &dyn ObjectStore) -> BTreeMap<String, ObjectRef> {
    /// How many listing pages a case may take before it calls Storage broken.
    const MAX_PAGES: usize = 1000;

    let mut handles = BTreeMap::new();
    let mut token = None;
    for _ in 0..MAX_PAGES {
        let page = store
            .list(token.as_ref())
            .await
            .expect("listing a store must succeed");
        for object in page.objects {
            handles.insert(object.name, object.object_ref);
        }
        token = page.next;
        if token.is_none() {
            return handles;
        }
    }
    panic!("listing did not end within {MAX_PAGES} pages");
}

/// The handle Storage names one Container's object by (spec: FM-3).
pub(crate) async fn container_handle(
    store: &dyn ObjectStore,
    container_id: ContainerId,
) -> ObjectRef {
    let name = container_id.object_name();
    handles(store)
        .await
        .remove(&name)
        .unwrap_or_else(|| panic!("{name:?} must be in Storage"))
}

/// Replaces the object at one name with bytes of the case's own.
///
/// An unconditional write, which is what a Container's name and a Keyring
/// replica's name both take (spec: KL-14, FM-3).
pub(crate) async fn overwrite(store: &dyn ObjectStore, name: &str, bytes: Vec<u8>) {
    store
        .put(name, ByteStream::from(bytes))
        .await
        .unwrap_or_else(|error| panic!("overwriting {name:?} must succeed: {error}"));
}

/// The name of one replica of the Keyring generation a checkpoint selected
/// (spec: CP-10, FM-12).
pub(crate) fn replica_name(commitment: &KeyringCommitment, index_of: u16) -> String {
    let replica = ReplicaPosition::new(index_of, commitment.replica_count())
        .expect("a declared replica index is a valid position");
    ControlObjectName::keyring_replica(commitment.generation(), commitment.set_digest(), replica)
        .expect("a committed digest is a valid one")
        .to_string()
}
