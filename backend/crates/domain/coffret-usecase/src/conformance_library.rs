//! Reading a Library back out of Storage the way a device with no Index would.
//!
//! Every suite over a flow that writes user data ends at the same question:
//! having run, can somebody else open what is on Storage? Answering it from what
//! the run returned would prove nothing, so this walks the listing, opens the
//! committed Keyring under a purpose key derived from the Master Key alone,
//! unwraps the envelope it maps a Container to, and decodes the object — the
//! long way round, on purpose (spec: CP-10, KL-6, KL-7, FM-14).
//!
//! It is shared by the sync and freeze suites rather than written twice: what a
//! second device can open is one question, and two spellings of it could drift
//! into two answers.
//!
//! [`replace`] lives here for the same reason: the commit and fetch suites both
//! damage objects in place, and what replacing one means on a store whose names
//! are not unique should be settled once.

use std::collections::BTreeMap;

use coffret_format::{
    decode, decode_control_object, decode_journal_record, decode_keyring, unwrap_container_key,
    DecodedContainer, Purpose, PurposeKey,
};
use coffret_model::{
    ContainerId, ContainerKeyStatus, ControlObjectKind, ControlObjectName, Generation,
    JournalRecord, MasterKey, ObjectRef, ReplicaPosition,
};

use crate::byte_stream::ByteStream;
use crate::error_chains::every_link;
use crate::object_store::ObjectStore;

/// How many listing pages a case may take before it calls Storage broken.
const MAX_PAGES: usize = 1000;

/// What Storage holds, read the way a device with no Index would read it.
pub(crate) struct Library {
    handles: BTreeMap<String, ObjectRef>,
}

impl Library {
    /// Walks the whole listing.
    pub(crate) async fn read(store: &dyn ObjectStore) -> Self {
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
                return Self { handles };
            }
        }
        panic!("listing did not end within {MAX_PAGES} pages");
    }

    /// Whether one Container's object is still in the listing, which is to say
    /// not trashed — a different question from whether the Container is current
    /// (spec: FM-3).
    pub(crate) fn holds_container(&self, container_id: ContainerId) -> bool {
        self.handles.contains_key(&container_id.object_name())
    }

    /// How many links of the control-head chain Storage holds (spec: FM-12).
    ///
    /// A run refused before its commit point leaves this exactly where it
    /// found it, which is the whole of what a case needs to say about the head.
    pub(crate) fn heads(&self) -> usize {
        self.handles
            .keys()
            .filter(|name| {
                matches!(
                    ControlObjectName::parse(name),
                    Ok(ControlObjectName::Head { .. })
                )
            })
            .count()
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
            .unwrap_or_else(|error| {
                panic!("reading {name:?} back must succeed: {}", every_link(&error))
            })
            .into_bytes()
            .await
            .expect("the stream is as long as it claims")
    }

    /// Opens one Container the way another enrolled device would.
    ///
    /// It goes the long way round on purpose: the Keyring the record's
    /// commitment names is fetched and opened under a purpose key derived from
    /// the Master Key alone, the envelope it maps the Container to is unwrapped
    /// against that Container's own ID, and the object is decoded under the key
    /// that comes out (spec: CP-10, KL-6, KL-7, FM-14). Nothing the run reported
    /// is used, because what carrying data into the Library is worth is what a
    /// second device can open.
    pub(crate) async fn open(
        &self,
        store: &dyn ObjectStore,
        record: &JournalRecord,
        container_id: ContainerId,
        master_key: &MasterKey,
    ) -> DecodedContainer {
        let replica = ReplicaPosition::new(0, record.keyring().replica_count())
            .expect("a commitment declares at least one replica");
        let name = ControlObjectName::keyring_replica(
            record.keyring().generation(),
            record.keyring().set_digest(),
            replica,
        )
        .expect("a committed digest is a valid one");
        let spelling = name.to_string();

        let decoded = decode_control_object(
            &self.bytes(store, &spelling).await,
            &spelling,
            &PurposeKey::derive(master_key, Purpose::ControlKeyring),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{spelling:?} must open as a Keyring: {}",
                every_link(&error)
            )
        });
        assert_eq!(decoded.kind, ControlObjectKind::Keyring);

        let key_table = decode_keyring(&decoded.payload).unwrap_or_else(|error| {
            panic!("{spelling:?} must decode as FM-17: {}", every_link(&error))
        });
        let element = key_table
            .elements()
            .iter()
            .find(|element| element.container_id == container_id)
            .unwrap_or_else(|| {
                panic!("the committed Keyring must map Container {container_id} (spec: KL-7)")
            });
        let ContainerKeyStatus::Envelope(envelope) = element.key else {
            panic!("Container {container_id} was just committed, so its key is not lost");
        };

        let key = unwrap_container_key(
            &PurposeKey::derive(master_key, Purpose::ContainerWrap),
            &container_id,
            &envelope,
        )
        .expect("the envelope the commit wrote opens for its own Container");

        let object = container_id.object_name();
        decode(&self.bytes(store, &object).await, &key).unwrap_or_else(|error| {
            panic!(
                "{object:?} must decode as a Container: {}",
                every_link(&error)
            )
        })
    }

    /// The Journal record Storage holds at `generation`, decoded the way a
    /// device catching up would decode it (spec: FM-15).
    ///
    /// What a run returned is the record it meant to write; this is the one it
    /// did, which is what another device replays.
    pub(crate) async fn record(
        &self,
        store: &dyn ObjectStore,
        generation: Generation,
        master_key: &MasterKey,
    ) -> JournalRecord {
        let spelling = ControlObjectName::head(generation).to_string();
        let decoded = decode_control_object(
            &self.bytes(store, &spelling).await,
            &spelling,
            &PurposeKey::derive(master_key, Purpose::ControlJournal),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{spelling:?} must open as a Journal record: {}",
                every_link(&error)
            )
        });
        assert_eq!(decoded.kind, ControlObjectKind::Journal);
        decode_journal_record(&decoded.payload, decoded.generation).unwrap_or_else(|error| {
            panic!("{spelling:?} must decode as FM-15: {}", every_link(&error))
        })
    }
}

/// Makes `bytes` the one live object called `name`, on every store.
///
/// A [`put`](ObjectStore::put) alone replaces the object only where the store
/// keys objects by name. A store that mints identifiers — Google Drive among
/// them — does not make names unique, so a second put would leave the original
/// beside the replacement, and a reader that resolves the name could land on
/// either. A case that damages an object needs the damaged one to be what the
/// name resolves to, so every live object under the name is purged first, each
/// by the handle the listing gives it.
pub(crate) async fn replace(store: &dyn ObjectStore, name: &str, bytes: Vec<u8>) {
    let mut existing = Vec::new();
    let mut token = None;
    for _ in 0..MAX_PAGES {
        let page = store
            .list(token.as_ref())
            .await
            .expect("listing a store must succeed");
        existing.extend(
            page.objects
                .into_iter()
                .filter(|object| object.name == name)
                .map(|object| object.object_ref),
        );
        token = page.next;
        if token.is_none() {
            break;
        }
    }
    assert!(
        token.is_none(),
        "listing did not end within {MAX_PAGES} pages"
    );
    for object in &existing {
        store.purge(object).await.unwrap_or_else(|error| {
            panic!(
                "purging the object under {name:?} must succeed: {}",
                every_link(&error)
            )
        });
    }
    store
        .put(name, ByteStream::from(bytes))
        .await
        .unwrap_or_else(|error| panic!("storing {name:?} must succeed: {}", every_link(&error)));
}
