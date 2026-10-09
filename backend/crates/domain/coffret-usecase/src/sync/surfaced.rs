use std::sync::Arc;

use coffret_model::{ContainerId, EntryPath};

use crate::descent_error::DescentError;

/// A file the sync found needing work it does not do, or a file whose Entry
/// left the Library and what the sync did about the copy on this device.
///
/// Every shape is a real finding and none is an error: a sync that has no Pack
/// path and no deletion path still has to say which files it left alone.
/// Silently skipping a file needing an update is the one outcome the rule
/// forbids outright — it makes the user believe stale content is safely backed
/// up (spec: PK-14). A file a deletion reached on this device's disk is said
/// the same way, whether the run moved it to the trash or kept it, because a
/// file that went from a person's folder without a word is the other half of
/// that same silence (spec: EP-15).
///
/// The Entry Path travels in the value because the caller is what decides what
/// to do about it. It never travels into a diagnostic event.
///
/// There is deliberately no `PartialEq`, for the reason
/// [`Disposal`](crate::sync::Disposal) has none: one shape carries the refusal
/// a move to the trash met, and a caller decides from the variant and the path
/// rather than by comparing two refusals.
#[derive(Debug, Clone)]
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
    /// The Entry this device materialized here left the Library, the file still
    /// held what this device last made it match, and the run moved it to the
    /// desktop's trash (spec: EP-15).
    ///
    /// Work done rather than work left: the row is gone with the file, so the
    /// path is outside this device's scope again and nothing more is said of it.
    /// The trash is where a person who meant to keep the file takes it back
    /// from.
    MovedToTrash {
        /// Where in the Library the file stood.
        path: EntryPath,
    },
    /// The Entry this device materialized here left the Library, and the file
    /// has changed since this device last made it match (spec: EP-15).
    ///
    /// Kept where it is and never carried back in: the change is the person's,
    /// to a file the Library no longer has, and neither the trash nor a new
    /// Entry is a decision a sync may make for them. Reported by every run until
    /// they move it — at another path it is an ordinary new file — or remove it.
    KeptEdited {
        /// Where in the Library the file stood.
        path: EntryPath,
    },
    /// The file was due to go to the trash, and the move was refused — by the
    /// root, the way down, or the trash itself (spec: EP-15).
    ///
    /// Left exactly where it is — never deleted outright in the trash's place —
    /// and its row with it, so the next run finds it as this one did and tries
    /// again. Nothing else in the run is held up by it.
    MoveToTrashRefused {
        /// Where in the Library the file stands.
        path: EntryPath,
        /// What the move was refused with: a root that could not be vouched for
        /// (spec: EP-13), a folder on the way down that is not one, or what
        /// the trash or the disk answered.
        ///
        /// Shared rather than owned so the finding can be cloned; a refusal is
        /// reported, never changed.
        cause: Arc<DescentError>,
    },
}

impl Surfaced {
    /// Where in the Library the file this finding is about stands, or stood.
    pub fn path(&self) -> &EntryPath {
        match self {
            Self::PackResident { path, .. }
            | Self::DeletedLocally { path }
            | Self::MovedToTrash { path }
            | Self::KeptEdited { path }
            | Self::MoveToTrashRefused { path, .. } => path,
        }
    }
}
