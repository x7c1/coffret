use coffret_model::{ContainerSummary, ControlObjectName, EntryLocation, IndexCheckpoint};

/// The whole Library-wide content of an Index, as an Index Snapshot carries it.
///
/// A Snapshot holds the Index of the whole Library — every current Entry and
/// its Container, including Entries under subtrees the writing device does not
/// map — and no device state at all: no local root mappings, no local paths, no
/// record of which Entries a device has materialized, no spool locations
/// (spec: CK-7, EP-9, EP-10). That is what lets two devices laid out
/// differently restore identical content from one Snapshot, and it is why
/// [`Index::restore`](crate::Index::restore) replaces this and leaves device
/// state alone.
///
/// One field is not Snapshot content: [`adopted_from`](Self::adopted_from) is
/// this Index's own provenance, and encoding it would put a device-local fact
/// into an object CK-7 says carries none.
///
/// It is a domain value here. Encoding it, encrypting it under a purpose key,
/// and framing it as a control object is the format layer's business
/// (spec: FM-11, RV-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotContent {
    /// The committed Library state this content stands at (spec: CK-1 to CK-3).
    pub checkpoint: IndexCheckpoint,
    /// The checkpoint object this content was adopted from, when it was adopted
    /// rather than replayed.
    ///
    /// A device catching up adopts the newest valid checkpoint when that is
    /// newer than its own Index — an ordinary Snapshot under an `idx-` name or
    /// an activation one under a `head-` name, which are equally candidates
    /// (spec: CK-9, RV-1). Recording which one it took is the Index's own
    /// provenance and not part of what a Snapshot's payload carries, so a
    /// device that has only ever replayed records holds `None`.
    pub adopted_from: Option<ControlObjectName>,
    /// The current Containers, ordered by Container ID.
    pub containers: Vec<ContainerSummary>,
    /// Every current Entry and where it lives, ordered by Entry Path bytes.
    pub entries: Vec<EntryLocation>,
}

impl SnapshotContent {
    /// The same content in the order [`Index::snapshot`](crate::Index::snapshot)
    /// reports it: Containers by ID, Entries by the canonical bytes of their
    /// Entry Path.
    ///
    /// Ordering by those bytes is ordering as EP-3 defines it — lexicographic
    /// over the canonical UTF-8 and independent of locale — so two devices
    /// canonicalize one Library's content identically and a Snapshot they each
    /// encode compares as one value.
    pub fn canonical(mut self) -> Self {
        self.containers.sort_by_key(|container| container.id);
        self.entries
            .sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
        self
    }
}
