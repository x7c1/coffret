use std::collections::BTreeMap;

use coffret_format::{ContainerOutline, Header};
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

/// Where one Container's chunk sequence starts in its object (spec: FM-2).
///
/// Read the way a partial fetch reads it: the header's 32 plaintext bytes say
/// how long the meta section behind them is, and everything past the two is
/// chunks. A case that wants to damage a *chunk* rather than the front of the
/// object needs the number, and works it out from the object exactly as the flow
/// does.
pub(crate) async fn body_start(store: &dyn ObjectStore, object: &ObjectRef) -> u64 {
    let front = store
        .get(object, Some(0..Header::LEN as u64))
        .await
        .expect("reading a Container's header must succeed")
        .into_bytes()
        .await
        .expect("draining a header must succeed");
    ContainerOutline::prefix_len(&front).expect("a committed Container has a valid header")
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
