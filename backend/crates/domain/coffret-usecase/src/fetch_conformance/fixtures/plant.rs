use coffret_format::{
    encode, generate_container_id, wrap_container_key, EncodeRequest, EntrySource, Purpose,
};
use coffret_model::{
    ContainerAddition, ContainerId, ContainerKey, ContainerKind, ContainerSummary, ContentHash,
    EntryMetadata, EntryPath, Mtime,
};

use crate::commit::{commit_batch, CommitRequest, PreparedAddition, PreparedBatch};
use crate::fetch::LibraryKeys;
use crate::fetch_conformance::fixtures::keys::purpose_key;
use crate::fetch_conformance::fixtures::objects::overwrite;
use crate::fetch_conformance::fixtures::runs::policy;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// Commits a Container of the suite's own making, the way another device would
/// have.
///
/// Two cases need a Library state no sync produces — an object at a Container's
/// name that is not a Container, and one whose entry table disagrees with the
/// record — so they commit it directly. Everything else about the state is real:
/// the envelope is a real wrap of a real key, the record is a real commit, and
/// the ciphertext hash is the hash of the bytes actually stored, so the fetch
/// gets all the way to the check the case is about.
pub(crate) async fn plant(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &LibraryKeys,
    planted: Planted<'_>,
) -> ContainerId {
    let container_id = generate_container_id().expect("the OS CSPRNG is available");
    let ciphertext = planted.ciphertext(container_id);
    overwrite(store, &container_id.object_name(), ciphertext.clone()).await;

    let entry = EntryMetadata {
        path: EntryPath::new(planted.path),
        offset: 0,
        size: planted.content.len() as u64,
        mtime: planted.mtime,
        hash: ContentHash::from_bytes(*blake3::hash(planted.content).as_bytes()),
        derived_from: None,
        mime: None,
    };
    let envelope = wrap_container_key(
        &purpose_key(Purpose::ContainerWrap),
        &container_id,
        &ContainerKey::from_bytes(PLANTED_KEY),
    )
    .expect("wrapping a Container Key must succeed");

    let batch = PreparedBatch::adding(vec![PreparedAddition::new(
        ContainerAddition {
            container: ContainerSummary {
                id: container_id,
                kind: ContainerKind::OneFile,
                // The hash of what is really stored, so the fetch's first check
                // passes and the case reaches the one it is about (spec: FM-15).
                ciphertext_hash: ContentHash::from_bytes(*blake3::hash(&ciphertext).as_bytes()),
                ciphertext_len: ciphertext.len() as u64,
                object_ref: None,
            },
            entries: vec![entry],
        },
        envelope,
    )]);

    commit_batch(CommitRequest::new(store, index, keys.control(), batch).with_policy(policy()))
        .await
        .unwrap_or_else(|error| panic!("committing a planted Container must succeed: {error}"));
    container_id
}

/// The Container Key every planted Container is sealed under.
const PLANTED_KEY: [u8; 32] = [0x11; 32];

/// One Container a case plants, and how its object is meant to disagree with its
/// record.
pub(crate) struct Planted<'a> {
    /// Where in the Library the Entry stands.
    pub(crate) path: &'a str,
    /// The content the record's entry table describes.
    pub(crate) content: &'a [u8],
    /// The Entry's modification time.
    pub(crate) mtime: Mtime,
    /// Whether the object at the Container's name is a real Container at all.
    pub(crate) real: bool,
    /// The content the object actually holds, where that differs from what the
    /// record says.
    pub(crate) actual_content: Option<&'a [u8]>,
}

impl Planted<'_> {
    /// The bytes that go to Storage under the Container's name.
    fn ciphertext(&self, container_id: ContainerId) -> Vec<u8> {
        if !self.real {
            return format!("not a Container at all, for {container_id}").into_bytes();
        }
        let content = self.actual_content.unwrap_or(self.content);
        let entries = [EntrySource::new(
            EntryPath::new(self.path),
            self.mtime,
            content,
        )];
        encode(&EncodeRequest::new(
            container_id,
            ContainerKind::OneFile,
            &ContainerKey::from_bytes(PLANTED_KEY),
            &entries,
        ))
        .expect("encoding a Container must succeed")
        .bytes()
        .to_vec()
    }
}

/// A moment in the past to stamp a planted Entry with.
pub(crate) const OLDER: i64 = 1_600_000_000;
