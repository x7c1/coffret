use std::path::PathBuf;

use coffret_model::{
    CiphertextLenClaim, ContainerAddition, ContainerId, ContainerKind, ContainerSummary,
    ContentHash, EntryMetadata, KeyEnvelope, ObjectRef,
};

use crate::commit::{
    commit_batch, CommitError, CommitFailure, CommitOutcome, CommitPolicy, CommitRequest,
    ControlKeys, DegradedReport, PreparedAddition, PreparedBatch,
};
use crate::device_state::{DeviceTime, LocalObservation};
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::progress::Progress;

/// One Container encoded and written to the spool, waiting to go up.
///
/// A sync draws one of these per changed file and a freeze draws one per Pack,
/// and past the point where the ciphertext exists the two are the same thing: an
/// object to upload, an addition to commit, and a set of Containers the addition
/// supersedes. So the shape is one type and the flows differ only in how they
/// fill it.
///
/// One of these exists only for a spool that was finished. A spool step returns
/// it as its last act, after the file is flushed and the pending row naming it
/// says [`Spooled`](crate::device_state::SpoolState::Spooled) — so the
/// `Vec` a run builds out of them, which is what the upload, the verification,
/// and the commit all act on, can hold no unfinished spool at all.
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
    /// How many bytes the object is, as this device measured it while writing
    /// it — which is what the record it commits claims (spec: FM-15).
    pub(crate) ciphertext_len: CiphertextLenClaim,
    /// The MD5 of the same bytes, as lowercase hex, for the provider's own
    /// comparison.
    pub(crate) provider_digest: String,
    /// Where the object went, once it has been uploaded.
    pub(crate) object_ref: Option<ObjectRef>,
    /// When this device announced the spool, which is what the pending row
    /// naming it records and keeps through every later update.
    pub(crate) announced_at: DeviceTime,
    /// The Containers this one supersedes, which the batch removes
    /// (spec: CP-14).
    ///
    /// A sync's replacement supersedes the one-file Container that held the
    /// Entry; a freeze's Pack supersedes every one-file Container it absorbed;
    /// a newly imported file supersedes nothing (spec: PK-7).
    pub(crate) replaces: Vec<ContainerId>,
    /// Whether the Entries it holds are files this device put into it from its
    /// own disk — which is what its commit records as materialized, and what
    /// the pending row naming it says for a later completion (spec: OC-7,
    /// EP-10).
    ///
    /// True for a sync's one-file Container and a freeze's Pack, which are
    /// built out of local files. False for a Pack rebuilt by
    /// read-modify-replace (spec: PK-10), whose Entries came off Storage.
    pub(crate) materializes: bool,
}

impl SpooledContainer {
    /// What the Journal record says about this Container, paired with the key
    /// that opens it (spec: CP-11, KL-7).
    ///
    /// The entry table is the one the encoder laid down, so it tiles the
    /// Container's plaintext stream and holds at least one Entry by
    /// construction (spec: FM-9, FM-10). A spool that somehow held one that did
    /// not would be this device's own doing, so the refusal travels as the
    /// commit's own rather than being unwrapped into a panic.
    pub(crate) fn addition(&self) -> Result<PreparedAddition, CommitError> {
        let container = ContainerSummary {
            id: self.container_id,
            kind: self.kind,
            ciphertext_hash: self.ciphertext_hash,
            ciphertext_len: self.ciphertext_len,
            // A cache and never evidence of membership (spec: FM-15): this
            // device holds the handle Storage answered its upload with, so a
            // reader can fetch the Container without listing first.
            object_ref: self.object_ref.clone(),
        };
        let addition = ContainerAddition::new(container, self.entries.clone())
            .map_err(|cause| CommitError::UnwritableControlValue { cause })?;

        Ok(PreparedAddition::new(addition, self.envelope))
    }

    /// The local files this device has in place for the Entries this Container
    /// holds (spec: EP-10).
    ///
    /// None at all for a Container rebuilt out of another one's bytes: what it
    /// carries forward never passed through this device's disk.
    pub(crate) fn materialized(
        &self,
        at: DeviceTime,
    ) -> impl Iterator<Item = LocalObservation> + '_ {
        let held: &[EntryMetadata] = if self.materializes {
            &self.entries
        } else {
            &[]
        };
        held.iter().map(move |entry| LocalObservation {
            path: entry.path.clone(),
            size: entry.extent.size(),
            mtime: entry.mtime,
            at,
        })
    }
}

/// Commits what a run uploaded and what it removes outright, or nothing where
/// it has neither.
///
/// A run with nothing to upload and nothing to remove commits nothing rather
/// than committing an empty batch: a Journal record is a generation, and
/// creating one for a batch that changes no Container would make every device
/// replay a record that says nothing (spec: CP-1).
///
/// `removals` are the Containers the batch takes out of the current set without
/// anything in it replacing them — a deletion's (spec: PK-9). What an upload
/// supersedes travels on the upload itself, in
/// [`replaces`](SpooledContainer::replaces), so a sync and a freeze pass none.
///
/// `degraded` is the finding a caller's own read of the committed Keyring left,
/// for the commit to speak for where it examines that same set (spec: KL-15). A
/// run that read nothing of the Keyring, or that ends here with nothing to
/// commit, hands over nothing and the caller's guard says its piece itself.
///
/// `progress` is the run's own, so the commit says how far it has got in the
/// same voice the upload before it did (see
/// [`Phase::Committing`](crate::Phase::Committing)).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn commit_spooled(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    now: DeviceTime,
    spooled: &[SpooledContainer],
    removals: &[ContainerId],
    degraded: Option<&DegradedReport>,
    progress: &dyn Progress,
) -> Result<Option<CommitOutcome>, CommitFailure> {
    if spooled.is_empty() && removals.is_empty() {
        return Ok(None);
    }
    let additions = spooled
        .iter()
        .map(SpooledContainer::addition)
        .collect::<Result<Vec<_>, CommitError>>()?;
    let batch = PreparedBatch::adding(additions)
        .removing(
            spooled
                .iter()
                .flat_map(|one| one.replaces.iter().copied())
                .chain(removals.iter().copied())
                .collect(),
        )
        .materializing(
            spooled
                .iter()
                .flat_map(|one| one.materialized(now))
                .collect(),
        );

    let request = CommitRequest::new(store, index, keys, batch)
        .with_policy(policy.clone())
        .speaking_for(degraded)
        .watched_by(progress);
    commit_batch(request).await.map(Some)
}
