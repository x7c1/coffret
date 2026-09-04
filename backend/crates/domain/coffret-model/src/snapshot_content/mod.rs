use crate::container_summary::ContainerSummary;
use crate::control_object_name::ControlObjectName;
use crate::entry_location::EntryLocation;
use crate::index_checkpoint::IndexCheckpoint;

mod new;

#[cfg(test)]
mod tests;

/// The whole Library-wide content of an Index, as an Index Snapshot carries it.
///
/// A Snapshot holds the Index of the whole Library — every current Entry and
/// its Container, including Entries under subtrees the writing device does not
/// map — and no device state at all: no local root mappings, no local paths, no
/// record of which Entries a device has materialized, no spool locations
/// (spec: CK-7, EP-9, EP-10). That is what lets two devices laid out
/// differently restore identical content from one Snapshot, and it is why
/// `Index::restore` replaces this and leaves device
/// state alone.
///
/// One field is not Snapshot content: [`adopted_from`](Self::adopted_from) is
/// this Index's own provenance, and encoding it would put a device-local fact
/// into an object CK-7 says carries none. It takes part in no rule here for the
/// same reason.
///
/// The rules the rest of it holds to are FM-16's, and they are held here rather
/// than at each reader: the Containers are in ID order, the Entries in the
/// canonical order of their Entry Path bytes (spec: EP-3), and every Entry
/// names a Container this content lists. A device restoring from one of these
/// therefore has nothing left to check. All three are [`new`](Self::new)'s to
/// state, so everything this type answers below answers without a refusal to
/// report.
///
/// It is a domain value here. Encoding it, encrypting it under a purpose key,
/// and framing it as a control object is the format layer's business
/// (spec: FM-11, RV-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotContent {
    checkpoint: IndexCheckpoint,
    adopted_from: Option<ControlObjectName>,
    containers: Vec<ContainerSummary>,
    entries: Vec<EntryLocation>,
}

impl SnapshotContent {
    /// The same content, recorded as adopted from `object`.
    ///
    /// Provenance is the Index's own and takes part in no rule (spec: CK-7), so
    /// stamping it on a value that already holds cannot make it stop holding —
    /// which is why this answers a value rather than a `Result`.
    #[must_use]
    pub fn adopted_from_object(mut self, object: ControlObjectName) -> Self {
        self.adopted_from = Some(object);
        self
    }

    /// The committed Library state this content stands at
    /// (spec: CK-1, CK-2, CK-3).
    pub const fn checkpoint(&self) -> &IndexCheckpoint {
        &self.checkpoint
    }

    /// The checkpoint object this content was adopted from, when it was adopted
    /// rather than replayed.
    ///
    /// A device catching up adopts the newest valid checkpoint when that is
    /// newer than its own Index — an ordinary Snapshot under an `idx-` name or
    /// an activation one under a `head-` name, which are equally candidates
    /// (spec: CK-9, RV-1). Recording which one it took is the Index's own
    /// provenance and not part of what a Snapshot's payload carries, so a
    /// device that has only ever replayed records holds `None`.
    pub const fn adopted_from(&self) -> Option<&ControlObjectName> {
        self.adopted_from.as_ref()
    }

    /// The current Containers, ordered by Container ID.
    pub fn containers(&self) -> &[ContainerSummary] {
        &self.containers
    }

    /// Every current Entry and where it lives, ordered by Entry Path bytes.
    pub fn entries(&self) -> &[EntryLocation] {
        &self.entries
    }

    /// The four halves, for the restore that consumes all of them.
    pub fn into_parts(
        self,
    ) -> (
        IndexCheckpoint,
        Option<ControlObjectName>,
        Vec<ContainerSummary>,
        Vec<EntryLocation>,
    ) {
        (
            self.checkpoint,
            self.adopted_from,
            self.containers,
            self.entries,
        )
    }
}
