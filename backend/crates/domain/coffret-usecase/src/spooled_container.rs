use std::path::PathBuf;

use coffret_model::{
    ContainerAddition, ContainerId, ContainerKind, ContainerSummary, ContentHash, EntryMetadata,
    KeyEnvelope, ObjectRef,
};

use crate::commit::{
    commit_batch, CommitError, CommitOutcome, CommitPolicy, CommitRequest, ControlKeys,
    PreparedAddition, PreparedBatch,
};
use crate::device_state::{DeviceTime, LocalObservation};
use crate::index::Index;
use crate::object_store::ObjectStore;

/// One Container encoded and written to the spool, waiting to go up.
///
/// A sync draws one of these per changed file and a freeze draws one per Pack,
/// and past the point where the ciphertext exists the two are the same thing: an
/// object to upload, an addition to commit, and a set of Containers the addition
/// displaces. So the shape is one type and the flows differ only in how they
/// fill it.
///
/// It carries both halves of what a commit needs — what the Journal record says
/// about the Container and the envelope the Keyring maps it to (spec: CP-11,
/// KL-7) — plus the two digests, which answer different questions and are never
/// interchangeable. The BLAKE3 is what the record carries and what a device
/// verifies the stored object against (spec: FM-15). The MD5 is a
/// provider-scoped token, good for asking one provider whether the bytes it
/// stored are the bytes that were sent and good for nothing else.
#[derive(Debug, Clone)]
pub(crate) struct SpooledContainer {
    /// The Container the spool holds.
    pub(crate) container_id: ContainerId,
    /// Which kind of user-data Container it is (spec: PK-15).
    pub(crate) kind: ContainerKind,
    /// Where the ciphertext sits on this device.
    pub(crate) spool_path: PathBuf,
    /// The Entries inside it, in the order they occupy the stream (spec: FM-9).
    pub(crate) entries: Vec<EntryMetadata>,
    /// The envelope the next Keyring generation maps the Container to
    /// (spec: FM-14, KL-7).
    pub(crate) envelope: KeyEnvelope,
    /// The BLAKE3-256 of the stored object (spec: FM-15).
    pub(crate) ciphertext_hash: ContentHash,
    /// How many bytes the object is.
    pub(crate) ciphertext_len: u64,
    /// The MD5 of the same bytes, as lowercase hex, for the provider's own
    /// comparison.
    pub(crate) provider_digest: String,
    /// Where the object went, once it has been uploaded.
    pub(crate) object_ref: Option<ObjectRef>,
    /// The Containers this one displaces, which the batch removes
    /// (spec: CP-14).
    ///
    /// A sync's replacement displaces the one-file Container that held the
    /// Entry; a freeze's Pack displaces every one-file Container it absorbed;
    /// a newly imported file displaces nothing (spec: PK-7).
    pub(crate) replaces: Vec<ContainerId>,
}

impl SpooledContainer {
    /// What the Journal record says about this Container, paired with the key
    /// that opens it (spec: CP-11, KL-7).
    pub(crate) fn addition(&self) -> PreparedAddition {
        PreparedAddition::new(
            ContainerAddition {
                container: ContainerSummary {
                    id: self.container_id,
                    kind: self.kind,
                    ciphertext_hash: self.ciphertext_hash,
                    ciphertext_len: self.ciphertext_len,
                    // A cache and never evidence of membership (spec: FM-15):
                    // this device holds the handle Storage answered its upload
                    // with, so a reader can fetch the Container without listing
                    // first.
                    object_ref: self.object_ref.clone(),
                },
                entries: self.entries.clone(),
            },
            self.envelope,
        )
    }

    /// The local files this device has in place for the Entries this Container
    /// holds (spec: EP-10).
    pub(crate) fn materialized(
        &self,
        at: DeviceTime,
    ) -> impl Iterator<Item = LocalObservation> + '_ {
        self.entries.iter().map(move |entry| LocalObservation {
            path: entry.path.clone(),
            size: entry.size,
            mtime: entry.mtime,
            at,
        })
    }
}

/// Commits what a run uploaded, or nothing where it uploaded nothing.
///
/// A run with nothing to upload commits nothing rather than committing an empty
/// batch: a Journal record is a generation, and spending one on a batch that
/// changes no Container would make every device replay a record that says
/// nothing (spec: CP-1).
pub(crate) async fn commit_spooled(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    now: DeviceTime,
    spooled: &[SpooledContainer],
) -> Result<Option<CommitOutcome>, CommitError> {
    if spooled.is_empty() {
        return Ok(None);
    }
    let batch = PreparedBatch::adding(spooled.iter().map(SpooledContainer::addition).collect())
        .removing(
            spooled
                .iter()
                .flat_map(|one| one.replaces.iter().copied())
                .collect(),
        )
        .materializing(
            spooled
                .iter()
                .flat_map(|one| one.materialized(now))
                .collect(),
        );

    let request = CommitRequest::new(store, index, keys, batch).with_policy(policy.clone());
    Ok(Some(commit_batch(request).await?))
}
