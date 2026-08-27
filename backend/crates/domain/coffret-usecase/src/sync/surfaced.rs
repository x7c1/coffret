use coffret_model::{ContainerId, EntryPath};

/// A file the sync found needing work it does not do.
///
/// Both shapes are real findings and neither is an error: a sync that has no
/// Pack path and no deletion path still has to say which files it left alone.
/// Silently skipping a file needing an update is the one outcome the rule
/// forbids outright — it makes the user believe stale content is safely backed
/// up (spec: PK-14).
///
/// The Entry Path travels in the value because the caller is what decides what
/// to do about it. It never travels into a log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Surfaced {
    /// The file changed, and its current Entry lives in a Pack.
    ///
    /// Replacing it means read-modify-replace over that Pack — reading and
    /// verifying every Entry in it, carrying the unchanged ones forward, and
    /// committing the replacement in one batch (spec: PK-10, PK-12). That is
    /// the half of `update` this flow does not do, and not `freeze`'s to do
    /// instead: an Entry already in a Pack is never eligible for one
    /// (spec: PK-1, PK-11). So the Pack is left byte-for-byte as it is.
    PackResident {
        /// Where in the Library the changed file stands.
        path: EntryPath,
        /// The Pack holding its current Entry.
        container_id: ContainerId,
    },
    /// A file this device had materialized is gone from disk (spec: EP-10).
    ///
    /// Reported and not acted on: removing the Entry from the Library is a
    /// deletion the user asks for explicitly, never something a sync infers
    /// from a missing file. The device-local row stays as it is, so the finding
    /// is reported again by every later run until somebody acts on it.
    DeletedLocally {
        /// Where in the Library the missing file stood.
        path: EntryPath,
    },
}
