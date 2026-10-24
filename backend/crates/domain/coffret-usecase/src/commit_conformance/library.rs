use std::collections::BTreeMap;

use coffret_format::{
    decode_control_object, decode_index_snapshot, decode_journal_record, decode_keyring,
    encode_control_object, encode_keyring, keyring_set_digest, ControlEncodeRequest, Purpose,
};
use coffret_model::{
    ContainerId, ControlObjectKind, ControlObjectName, Generation, JournalRecord, KeyTable,
    KeyringCommitment, KeyringElement, MasterKeyEpoch, ObjectRef, ReplicaPosition, SnapshotContent,
};

use crate::byte_stream::ByteStream;
use crate::commit_conformance::fixtures::{container_id, envelope, purpose_key};
use crate::error_chains::every_link;
use crate::object_store::ObjectStore;

/// What Storage holds, read the way a device with no Index would read it.
///
/// The cases assert against this rather than against what the flow reported,
/// because what a commit is worth is what another device would find: a record
/// that decodes to the batch, a Keyring set that is complete and valid, a
/// checkpoint under the one name its head gives it. Every check here goes
/// through the format layer with keys derived independently of the flow.
pub(super) struct Library {
    /// Every live object Storage listed under each name, in listing order. One,
    /// everywhere a Library is sound but for a Keyring replica a repair wrote
    /// beside a damaged copy; a store that mints identifiers does not make
    /// names unique, so this is what a case reads to find out that it was not.
    listed: BTreeMap<String, Vec<ObjectRef>>,
}

/// How many pages a listing may take before the suite calls Storage broken.
const MAX_PAGES: usize = 1000;

impl Library {
    /// Walks the whole listing.
    pub(super) async fn read(store: &dyn ObjectStore) -> Self {
        let mut listed: BTreeMap<String, Vec<ObjectRef>> = BTreeMap::new();
        let mut token = None;
        let mut pages = 0;
        loop {
            let page = store
                .list(token.as_ref())
                .await
                .expect("listing a store must succeed");
            for object in page.objects {
                listed
                    .entry(object.name)
                    .or_default()
                    .push(object.object_ref);
            }
            token = page.next;
            pages += 1;
            if token.is_none() {
                return Self { listed };
            }
            assert!(
                pages < MAX_PAGES,
                "listing did not end within {MAX_PAGES} pages"
            );
        }
    }

    /// Every object name in Storage, sorted.
    pub(super) fn names(&self) -> Vec<String> {
        self.listed.keys().cloned().collect()
    }

    /// Whether any live object's name starts with `prefix`.
    pub(super) fn holds_any(&self, prefix: &str) -> bool {
        self.listed.keys().any(|name| name.starts_with(prefix))
    }

    /// Whether one Container's object is still live (spec: FM-3).
    pub(super) fn holds_container(&self, container_id: ContainerId) -> bool {
        self.listed.contains_key(&container_id.object_name())
    }

    /// How many live objects Storage lists under one name.
    pub(super) fn count(&self, name: &ControlObjectName) -> usize {
        self.copies(name).len()
    }

    /// Every live object Storage lists under one name, in listing order.
    pub(super) fn copies(&self, name: &ControlObjectName) -> &[ObjectRef] {
        self.listed
            .get(&name.to_string())
            .map_or(&[], Vec::as_slice)
    }

    /// Whether Storage still lists an object under one name.
    pub(super) fn holds(&self, name: &ControlObjectName) -> bool {
        self.listed.contains_key(&name.to_string())
    }

    /// The handle Storage named one object by, which the case expects to be
    /// there — the one listed last, where it listed more than one.
    ///
    /// A case that wants a store to misbehave about *one* object needs the
    /// handle rather than the name, because a store that mints identifiers names
    /// nothing by its name — which is the same reason the flow itself carries a
    /// listing around (spec: FM-3, FM-12).
    pub(super) fn handle(&self, name: &ControlObjectName) -> ObjectRef {
        self.handle_named(&name.to_string())
    }

    /// The handle Storage listed under one name, given as it is listed, which
    /// the case expects to be there.
    pub(super) fn handle_named(&self, name: &str) -> ObjectRef {
        self.listed
            .get(name)
            .and_then(|copies| copies.last())
            .unwrap_or_else(|| panic!("{name:?} must be in Storage"))
            .clone()
    }

    /// The bytes of one object, which the case expects to be there.
    pub(super) async fn bytes(&self, store: &dyn ObjectStore, name: &str) -> Vec<u8> {
        Self::bytes_of(store, name, &self.handle_named(name)).await
    }

    /// The bytes of one object Storage listed under `name`.
    async fn bytes_of(store: &dyn ObjectStore, name: &str, object: &ObjectRef) -> Vec<u8> {
        store
            .get(object, None)
            .await
            .unwrap_or_else(|error| {
                panic!("reading {name:?} back must succeed: {}", every_link(&error))
            })
            .into_bytes()
            .await
            .expect("the stream is as long as it claims")
    }

    /// The Journal record committed at one generation (spec: FM-15).
    pub(super) async fn record(
        &self,
        store: &dyn ObjectStore,
        generation: Generation,
    ) -> JournalRecord {
        let name = ControlObjectName::head(generation);
        let spelling = name.to_string();
        let decoded = decode_control_object(
            &self.bytes(store, &spelling).await,
            &spelling,
            &purpose_key(Purpose::ControlJournal),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{spelling:?} must open as a Journal record: {}",
                every_link(&error)
            )
        });

        assert_eq!(decoded.kind, ControlObjectKind::Journal);
        assert_eq!(decoded.generation, generation);
        assert_eq!(decoded.replica, ReplicaPosition::SINGLE);
        decode_journal_record(&decoded.payload, generation).unwrap_or_else(|error| {
            panic!("{spelling:?} must decode as FM-15: {}", every_link(&error))
        })
    }

    /// The ordinary Index Snapshot checkpointing one head (spec: CK-10, FM-16).
    pub(super) async fn snapshot(
        &self,
        store: &dyn ObjectStore,
        generation: Generation,
    ) -> SnapshotContent {
        let name = ControlObjectName::index_snapshot(generation);
        let spelling = name.to_string();
        let decoded = decode_control_object(
            &self.bytes(store, &spelling).await,
            &spelling,
            &purpose_key(Purpose::ControlIndexSnapshot),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{spelling:?} must open as an Index Snapshot: {}",
                every_link(&error)
            )
        });

        assert_eq!(decoded.kind, ControlObjectKind::IndexSnapshot);
        assert_eq!(decoded.generation, generation);
        decode_index_snapshot(&decoded.payload, decoded.kind, generation)
            .unwrap_or_else(|error| {
                panic!("{spelling:?} must decode as FM-16: {}", every_link(&error))
            })
            .content
    }

    /// The Containers the committed Keyring maps, having checked the whole set.
    ///
    /// Completeness and validity together (spec: KL-1, KL-2): every replica
    /// index the commitment declares is present, each one opens under the
    /// Keyring purpose key with a header its name admits, each carries the same
    /// key table, and that key table digests to what the commitment and the names
    /// promise. A position Storage lists more than once is valid when any of its
    /// copies is (spec: KL-14), so each copy is tried and the position fails only
    /// when none of them passes.
    pub(super) async fn keyring(
        &self,
        store: &dyn ObjectStore,
        commitment: &KeyringCommitment,
    ) -> Vec<KeyringElement> {
        let mut agreed: Option<Vec<KeyringElement>> = None;
        for index in 0..commitment.replica_count() {
            let replica = ReplicaPosition::new(index, commitment.replica_count())
                .expect("a declared replica index is a valid position");
            let name = ControlObjectName::keyring_replica(
                commitment.generation(),
                commitment.set_digest(),
                replica,
            )
            .expect("a committed digest is a valid one");
            let spelling = name.to_string();
            let copies = self.copies(&name);
            assert!(!copies.is_empty(), "{spelling:?} must be in Storage");

            let mut refusals = Vec::new();
            let mut valid = None;
            for object in copies {
                let bytes = Self::bytes_of(store, &spelling, object).await;
                match valid_replica(&bytes, &spelling, replica, commitment) {
                    Ok(key_table) => {
                        valid = Some(key_table);
                        break;
                    }
                    Err(refusal) => refusals.push(refusal),
                }
            }
            let key_table = valid.unwrap_or_else(|| {
                panic!(
                    "no copy of {spelling:?} is a valid replica: {}",
                    refusals.join("; ")
                )
            });

            match &agreed {
                Some(held) => assert_eq!(
                    held.as_slice(),
                    key_table.elements(),
                    "every replica of one generation carries one key table",
                ),
                None => agreed = Some(key_table.elements().to_vec()),
            }
        }
        agreed.expect("a commitment declares at least one replica")
    }

    /// Puts a Container's ciphertext where its ID says it goes (spec: FM-3).
    ///
    /// The bytes are not a Container and deliberately so: a commit never opens
    /// one (spec: CP-11), so a flow that needed them to be real would be doing
    /// something the protocol forbids. What the cases need is an object at the
    /// name, so that a removal has something to trash.
    pub(super) async fn upload_container(store: &dyn ObjectStore, container_id: ContainerId) {
        store
            .put(
                &container_id.object_name(),
                ByteStream::from(format!("ciphertext of {container_id}").into_bytes()),
            )
            .await
            .expect("storing a Container must succeed");
    }
}

/// The key table one copy of a replica carries, or why the copy is not a valid
/// replica of `commitment` at `replica` (spec: KL-1, FM-17).
fn valid_replica(
    bytes: &[u8],
    spelling: &str,
    replica: ReplicaPosition,
    commitment: &KeyringCommitment,
) -> Result<KeyTable, String> {
    let decoded = decode_control_object(bytes, spelling, &purpose_key(Purpose::ControlKeyring))
        .map_err(|error| format!("it does not open as a Keyring: {}", every_link(&error)))?;
    if decoded.kind != ControlObjectKind::Keyring {
        return Err(format!("it opens as a {:?}", decoded.kind));
    }
    if decoded.replica != replica {
        return Err(format!("its header says {:?}", decoded.replica));
    }
    let key_table = decode_keyring(&decoded.payload)
        .map_err(|error| format!("it does not decode as FM-17: {}", every_link(&error)))?;
    let digest = keyring_set_digest(&key_table).expect("a key table always digests");
    if digest != commitment.set_digest() {
        return Err("it carries a key table its name does not promise".to_owned());
    }
    Ok(key_table)
}

/// The Containers a key table covers, in the order the wire form fixes.
pub(super) fn mapped(elements: &[KeyringElement]) -> Vec<ContainerId> {
    elements
        .iter()
        .map(|element| element.container_id)
        .collect()
}

/// The name one declared position of a committed set is stored under
/// (spec: FM-12).
pub(super) fn replica_name(commitment: &KeyringCommitment, index: u16) -> ControlObjectName {
    let replica = ReplicaPosition::new(index, commitment.replica_count())
        .expect("a declared replica index is a valid position");
    ControlObjectName::keyring_replica(commitment.generation(), commitment.set_digest(), replica)
        .expect("a committed digest is a valid one")
}

/// Takes one replica of a committed set out of Storage (spec: KL-5).
///
/// Through the recoverable removal, which is what object loss looks like from
/// the Library's side: the name leaves the listing, and a walk that goes looking
/// for it finds nothing there (spec: KL-1).
pub(super) async fn lose_replica(
    store: &dyn ObjectStore,
    commitment: &KeyringCommitment,
    index: u16,
) {
    let name = replica_name(commitment, index);
    remove_every_copy(store, &name).await;
}

/// Trashes every object Storage lists under one name.
///
/// Every one rather than the last listed, because a store that mints
/// identifiers can hold several under a replica name (spec: KL-14), and a case
/// that means a position to be gone, or replaced, means all of them.
async fn remove_every_copy(store: &dyn ObjectStore, name: &ControlObjectName) {
    let library = Library::read(store).await;
    let copies = library.copies(name);
    assert!(!copies.is_empty(), "{name} must be in Storage");
    for object in copies {
        store
            .trash(object)
            .await
            .unwrap_or_else(|error| panic!("removing {name} must succeed: {}", every_link(&error)));
    }
}

/// Replaces one replica with bytes that are no control object at all.
///
/// Replaces and does not add: every object under the name goes first, so a
/// store that mints identifiers is left with the damaged copy alone, as one
/// keyed by name is. A damaged copy *beside* a valid one is another state, and
/// another case's (spec: KL-14).
///
/// The other half of losing one: the name is still in the listing and the object
/// behind it is not a replica this Library can read a key table from — which is
/// the degradation a walk finds only by reading, and exactly what makes the full
/// walk worth its cost (spec: KL-1, KL-5).
pub(super) async fn mangle_replica(
    store: &dyn ObjectStore,
    commitment: &KeyringCommitment,
    index: u16,
) {
    let name = replica_name(commitment, index);
    remove_every_copy(store, &name).await;
    store
        .put(
            &name.to_string(),
            ByteStream::from(b"not a Keyring replica at all".to_vec()),
        )
        .await
        .unwrap_or_else(|error| panic!("overwriting {name} must succeed: {}", every_link(&error)));
}

/// Replaces one replica with a Keyring that opens and carries another key table.
///
/// Every object under the name goes first, for the reason
/// [`mangle_replica`] gives.
///
/// The subtlest way a replica is not valid, and the one an authenticating reader
/// would otherwise accept: the object decrypts, its header agrees with its name,
/// and the key table inside digests to something its name does not promise
/// (spec: KL-1, CP-10, FM-17). Only the third check catches it.
pub(super) async fn misdigest_replica(
    store: &dyn ObjectStore,
    commitment: &KeyringCommitment,
    index: u16,
) {
    let name = replica_name(commitment, index);
    // A key table no case commits, so its digest is not the one the commitment
    // named whatever the case put in the Library.
    let other = KeyTable::canonical(vec![KeyringElement::envelope(
        container_id(0xee),
        envelope(0xee),
    )])
    .expect("a one-element key table is canonical");
    assert_ne!(
        keyring_set_digest(&other).expect("a key table always digests"),
        commitment.set_digest(),
        "the case needs a key table the commitment did not name",
    );

    let payload =
        encode_keyring(&other, MasterKeyEpoch::FIRST).expect("a key table encodes as FM-17");
    let object = encode_control_object(&ControlEncodeRequest::new(
        &name,
        ControlObjectKind::Keyring,
        &purpose_key(Purpose::ControlKeyring),
        &payload,
    ))
    .expect("framing a Keyring replica must succeed");
    remove_every_copy(store, &name).await;
    store
        .put(&name.to_string(), ByteStream::from(object.bytes().to_vec()))
        .await
        .unwrap_or_else(|error| panic!("overwriting {name} must succeed: {}", every_link(&error)));
}
