use coffret_format::{
    encode, generate_container_id, wrap_container_key, EncodeRequest, EntrySource, Purpose,
};
use coffret_model::{
    ContainerAddition, ContainerId, ContainerKey, ContainerKind, ContainerSummary, ContentHash,
    EntryExtent, EntryMetadata, Mtime,
};

use crate::ciphertext_len_claims::ciphertext_len;
use crate::commit::{commit_batch, CommitRequest, PreparedAddition, PreparedBatch};
use crate::entry_paths::entry_path;
use crate::error_chains::every_link;
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
        path: entry_path(planted.path),
        captured_path: entry_path(planted.path),
        extent: EntryExtent::from_start(planted.content.len() as u64)
            .expect("a fixture's content is shorter than the address space the format admits"),
        mtime: planted.mtime,
        btime: None,
        hash: ContentHash::from_bytes(*blake3::hash(planted.content).as_bytes()),
        derived_from: None,
        mime: None,
    };
    // An Entry of no bytes after the first, which the record's table holds
    // and the object's does not: entry number 1 of a table one Entry long.
    let unheld = planted.unheld.map(|path| EntryMetadata {
        path: entry_path(path),
        captured_path: entry_path(path),
        extent: EntryExtent::new(entry.extent.end(), 0)
            .expect("an Entry of no bytes at the end of a short stream"),
        hash: ContentHash::from_bytes(*blake3::hash(b"").as_bytes()),
        ..entry.clone()
    });
    let entries: Vec<EntryMetadata> = std::iter::once(entry).chain(unheld).collect();
    let envelope = wrap_container_key(
        &purpose_key(Purpose::ContainerWrap),
        &container_id,
        &ContainerKey::from_bytes(PLANTED_KEY),
    )
    .expect("wrapping a Container Key must succeed");

    let batch = PreparedBatch::adding(vec![PreparedAddition::new(
        ContainerAddition::new(
            ContainerSummary {
                id: container_id,
                kind: planted.kind(),
                // The hash of what is really stored, so the fetch's first check
                // passes and the case reaches the one it is about (spec: FM-15)
                // — unless the case is about that check.
                ciphertext_hash: planted.recorded_hash(&ciphertext),
                ciphertext_len: ciphertext_len(ciphertext.len() as u64),
                object_ref: None,
            },
            entries,
        )
        .expect("a fixture holds a table that tiles its Container's stream"),
        envelope,
    )]);

    commit_batch(CommitRequest::new(store, index, keys.control(), batch).with_policy(policy()))
        .await
        .unwrap_or_else(|error| {
            panic!(
                "committing a planted Container must succeed: {}",
                every_link(&error)
            )
        });
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
    /// A meta section length to write over the one the encoder produced.
    ///
    /// Four bytes of the header, which is all it takes: the field is plaintext
    /// and unauthenticated (spec: FM-2), so anyone who can write at the object's
    /// name can put any number there. `None` leaves the object as encoded.
    pub(crate) meta_len: Option<u32>,
    /// How many bytes to cut off the end of the encoded object before it is
    /// stored, where the object is to be shorter than its header says.
    ///
    /// The record measures and hashes what is stored, so the object is exactly
    /// the one the Library committed — and its authenticated header and meta
    /// section place chunks past its end. `None` stores the object whole.
    pub(crate) short_by: Option<usize>,
    /// Whether the Journal record carries a ciphertext hash other than the
    /// stored object's, where a case is about the read held to that hash
    /// (spec: CP-11, PK-22).
    ///
    /// Everything else about the object is true: it is a real Container under
    /// the key the record maps it to, holding the content the record names.
    pub(crate) misrecorded: bool,
    /// A second Entry the record's table holds, of no bytes, that the object's
    /// own table does not: where a case is about an entry number the
    /// Container does not reach (spec: FM-9, CP-11). The Container is a Pack
    /// where there is one, in the record and the object alike, since a
    /// one-file Container holds one Entry (spec: PK-15).
    pub(crate) unheld: Option<&'a str>,
}

impl Planted<'_> {
    /// The kind of Container the record and the object both say it is.
    fn kind(&self) -> ContainerKind {
        match self.unheld {
            Some(_) => ContainerKind::Pack,
            None => ContainerKind::OneFile,
        }
    }

    /// The ciphertext hash the Journal record carries for what is stored.
    fn recorded_hash(&self, ciphertext: &[u8]) -> ContentHash {
        let stored = blake3::hash(ciphertext);
        if !self.misrecorded {
            return ContentHash::from_bytes(*stored.as_bytes());
        }
        ContentHash::from_bytes(*blake3::hash(stored.as_bytes()).as_bytes())
    }
}

/// Where the meta section length sits in a Container header (spec: FM-2).
const META_LEN_RANGE: std::ops::Range<usize> = 28..32;

impl Planted<'_> {
    /// The bytes that go to Storage under the Container's name.
    fn ciphertext(&self, container_id: ContainerId) -> Vec<u8> {
        if !self.real {
            return format!("not a Container at all, for {container_id}").into_bytes();
        }
        let content = self.actual_content.unwrap_or(self.content);
        let entries = [EntrySource::new(entry_path(self.path), self.mtime, content)];
        let mut bytes = encode(&EncodeRequest::new(
            container_id,
            self.kind(),
            &ContainerKey::from_bytes(PLANTED_KEY),
            &entries,
        ))
        .expect("encoding a Container must succeed")
        .bytes()
        .to_vec();

        if let Some(meta_len) = self.meta_len {
            bytes[META_LEN_RANGE].copy_from_slice(&meta_len.to_be_bytes());
        }
        if let Some(short_by) = self.short_by {
            bytes.truncate(bytes.len() - short_by);
        }
        bytes
    }
}

/// A moment in the past to stamp a planted Entry with.
pub(crate) const OLDER: i64 = 1_600_000_000;
