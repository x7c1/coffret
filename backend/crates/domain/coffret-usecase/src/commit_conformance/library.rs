use std::collections::BTreeMap;

use coffret_format::{
    decode_control_object, decode_index_snapshot, decode_journal_record, decode_keyring,
    keyring_set_digest, Purpose,
};
use coffret_model::{
    ContainerId, ControlObjectKind, ControlObjectName, Generation, JournalRecord,
    KeyringCommitment, KeyringEntry, ObjectRef, ReplicaPosition, SnapshotContent,
};

use crate::byte_stream::ByteStream;
use crate::commit_conformance::fixtures::purpose_key;
use crate::object_store::ObjectStore;

/// What Storage holds, read the way a device with no Index would read it.
///
/// The cases assert against this rather than against what the flow reported,
/// because what a commit is worth is what another device would find: a record
/// that decodes to the batch, a Keyring set that is complete and valid, a
/// checkpoint under the one name its head gives it. Every check here goes
/// through the format layer with keys derived independently of the flow.
pub(super) struct Library {
    handles: BTreeMap<String, ObjectRef>,
}

/// How many pages a listing may take before the suite calls Storage broken.
const MAX_PAGES: usize = 1000;

impl Library {
    /// Walks the whole listing.
    pub(super) async fn read(store: &dyn ObjectStore) -> Self {
        let mut handles = BTreeMap::new();
        let mut token = None;
        let mut pages = 0;
        loop {
            let page = store
                .list(token.as_ref())
                .await
                .expect("listing a store must succeed");
            for object in page.objects {
                handles.insert(object.name, object.object_ref);
            }
            token = page.next;
            pages += 1;
            if token.is_none() {
                return Self { handles };
            }
            assert!(
                pages < MAX_PAGES,
                "listing did not end within {MAX_PAGES} pages"
            );
        }
    }

    /// Every object name in Storage, sorted.
    pub(super) fn names(&self) -> Vec<String> {
        self.handles.keys().cloned().collect()
    }

    /// Whether any live object's name starts with `prefix`.
    pub(super) fn holds_any(&self, prefix: &str) -> bool {
        self.handles.keys().any(|name| name.starts_with(prefix))
    }

    /// Whether one Container's object is still live (spec: FM-3).
    pub(super) fn holds_container(&self, container_id: ContainerId) -> bool {
        self.handles.contains_key(&container_id.object_name())
    }

    /// The bytes of one object, which the case expects to be there.
    async fn bytes(&self, store: &dyn ObjectStore, name: &str) -> Vec<u8> {
        let object = self
            .handles
            .get(name)
            .unwrap_or_else(|| panic!("{name:?} must be in Storage"));
        store
            .get(object, None)
            .await
            .unwrap_or_else(|error| panic!("reading {name:?} back must succeed: {error}"))
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
        .unwrap_or_else(|error| panic!("{spelling:?} must open as a Journal record: {error}"));

        assert_eq!(decoded.kind, ControlObjectKind::Journal);
        assert_eq!(decoded.generation, generation);
        assert_eq!(decoded.replica, ReplicaPosition::SINGLE);
        decode_journal_record(&decoded.payload, generation)
            .unwrap_or_else(|error| panic!("{spelling:?} must decode as FM-15: {error}"))
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
        .unwrap_or_else(|error| panic!("{spelling:?} must open as an Index Snapshot: {error}"));

        assert_eq!(decoded.kind, ControlObjectKind::IndexSnapshot);
        assert_eq!(decoded.generation, generation);
        decode_index_snapshot(&decoded.payload, decoded.kind)
            .unwrap_or_else(|error| panic!("{spelling:?} must decode as FM-16: {error}"))
            .content
    }

    /// The Containers the committed Keyring maps, having checked the whole set.
    ///
    /// Completeness and validity together (spec: KL-1, KL-2): every replica
    /// index the commitment declares is present, each one opens under the
    /// Keyring purpose key with a header its name admits, each carries the same
    /// mapping, and that mapping digests to what the commitment and the names
    /// promise.
    pub(super) async fn keyring(
        &self,
        store: &dyn ObjectStore,
        commitment: &KeyringCommitment,
    ) -> Vec<KeyringEntry> {
        let mut agreed: Option<Vec<KeyringEntry>> = None;
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

            let decoded = decode_control_object(
                &self.bytes(store, &spelling).await,
                &spelling,
                &purpose_key(Purpose::ControlKeyring),
            )
            .unwrap_or_else(|error| panic!("{spelling:?} must open as a Keyring: {error}"));
            assert_eq!(decoded.kind, ControlObjectKind::Keyring);
            assert_eq!(decoded.replica, replica);

            let mapping = decode_keyring(&decoded.payload)
                .unwrap_or_else(|error| panic!("{spelling:?} must decode as FM-17: {error}"));
            let digest = keyring_set_digest(&mapping).expect("a mapping always digests");
            assert_eq!(
                digest,
                commitment.set_digest(),
                "{spelling:?} carries a mapping its name does not promise",
            );

            match &agreed {
                Some(held) => assert_eq!(
                    held, &mapping.entries,
                    "every replica of one generation carries one mapping",
                ),
                None => agreed = Some(mapping.entries),
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

/// The Containers a mapping covers, in the order the wire form fixes.
pub(super) fn mapped(entries: &[KeyringEntry]) -> Vec<ContainerId> {
    entries.iter().map(|entry| entry.container_id).collect()
}
