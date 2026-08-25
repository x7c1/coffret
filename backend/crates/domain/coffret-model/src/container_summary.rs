use crate::container_id::ContainerId;
use crate::container_kind::ContainerKind;
use crate::content_hash::ContentHash;
use crate::object_ref::ObjectRef;

/// What the Index records about one current Container.
///
/// It is the Container-level half of what a Journal record's additions carry
/// (spec: CP-11) and what an Index Snapshot's `containers` lists (spec: FM-16)
/// — the ciphertext hash and the kind — kept so that neither answering "which
/// Containers are current" nor selecting `freeze` candidates has to open a
/// Container (spec: FM-9, PK-1, PK-15).
///
/// The Container's own meta section stays the authority on what it holds; this
/// is the copy replaying a record or restoring from a Snapshot leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerSummary {
    /// The identifier this Container carries for its whole life.
    pub id: ContainerId,
    /// Whether the Container was made one file at a time or by the pack policy
    /// (spec: PK-15).
    pub kind: ContainerKind,
    /// BLAKE3-256 of the Container's ciphertext, as its Journal record recorded
    /// it (spec: CP-11).
    pub ciphertext_hash: ContentHash,
    /// Length of the Container's ciphertext in bytes.
    pub ciphertext_len: u64,
    /// Storage's own identifier for this Container's object, when one is
    /// recorded.
    ///
    /// The value is the same whichever device reads it, and it is carried as a
    /// cache so that a fetch needs no listing first (spec: FM-15, FM-16). A
    /// Journal record and an Index Snapshot both carry it, so a device holds
    /// whatever the record it replayed or the Snapshot it restored from
    /// recorded; `None` says only that no writer recorded a reference — a
    /// name-keyed Storage, or a writer that had none — and the Container is
    /// then reached by the name its ID gives it (spec: FM-3).
    ///
    /// It is never evidence of membership: a listing re-derives it, and a device
    /// that cannot open the object it names falls back to the listing rather
    /// than failing (spec: FM-15).
    pub object_ref: Option<ObjectRef>,
}
