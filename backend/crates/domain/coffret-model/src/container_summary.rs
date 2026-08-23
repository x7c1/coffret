use crate::container_id::ContainerId;
use crate::container_kind::ContainerKind;
use crate::content_hash::ContentHash;
use crate::object_ref::ObjectRef;

/// What the Index records about one current Container.
///
/// It is the Container-level half of what a Journal record's additions carry
/// (spec: CP-11) — the ciphertext hash and the kind — kept so that neither
/// answering "which Containers are current" nor selecting `freeze` candidates
/// has to open a Container (spec: FM-9, PK-1, PK-15).
///
/// The Container's own meta section stays the authority on what it holds; this
/// is a copy the replay of a record leaves behind.
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
    /// Where the provider keeps this Container, when this device knows.
    ///
    /// A device that replayed a Journal record has never seen the object, so it
    /// holds `None` and reaches the Container by name — the name follows from
    /// the ID alone (spec: FM-3). A device that uploaded or fetched it keeps
    /// the handle, which spares a store that mints identifiers a lookup.
    pub object_ref: Option<ObjectRef>,
}
