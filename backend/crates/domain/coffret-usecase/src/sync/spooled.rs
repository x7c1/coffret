use std::path::PathBuf;

use coffret_model::{ContainerId, ContentHash, EntryMetadata, KeyEnvelope, ObjectRef};

/// One Container encoded and written to the spool, waiting to go up.
///
/// It carries both halves of what a commit needs — what the Journal record says
/// about the Container and the envelope the Keyring maps it to (spec: CP-11,
/// KL-7) — plus the two digests, which answer different questions and are never
/// interchangeable. The BLAKE3 is what the record carries and what a device
/// verifies the stored object against (spec: FM-15). The MD5 is a
/// provider-scoped token, good for asking one provider whether the bytes it
/// stored are the bytes that were sent and good for nothing else.
#[derive(Debug, Clone)]
pub(super) struct Spooled {
    /// The Container the spool holds.
    pub(super) container_id: ContainerId,
    /// Where the ciphertext sits on this device.
    pub(super) spool_path: PathBuf,
    /// The one Entry inside it (spec: FM-9).
    pub(super) entry: EntryMetadata,
    /// The envelope the next Keyring generation maps the Container to
    /// (spec: FM-14, KL-7).
    pub(super) envelope: KeyEnvelope,
    /// The BLAKE3-256 of the stored object (spec: FM-15).
    pub(super) ciphertext_hash: ContentHash,
    /// How many bytes the object is.
    pub(super) ciphertext_len: u64,
    /// The MD5 of the same bytes, as lowercase hex, for the provider's own
    /// comparison.
    pub(super) provider_digest: String,
    /// Where the object went, once it has been uploaded.
    pub(super) object_ref: Option<ObjectRef>,
    /// The one-file Container this one replaces, if any (spec: CP-14).
    pub(super) replaces: Option<ContainerId>,
}
