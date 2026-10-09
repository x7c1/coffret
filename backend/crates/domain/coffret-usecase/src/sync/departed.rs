use crate::device_state::RootMarkerId;
use crate::local_scan::SourceFile;

/// A file this device materialized whose Entry has left the Library, and which
/// still holds what this device last made it match (spec: EP-15).
///
/// What the run moves to the trash once the scan is over. The identity the
/// file's mapping expects of its root travels with it, because the move is a
/// write into the mapped folder and is made only where the root proves to be
/// the one the mapping was recorded against (spec: EP-13).
#[derive(Debug, Clone)]
pub(super) struct Departed {
    /// The file to move.
    pub(super) source: SourceFile,
    /// What the mapping the file was found under expects its root's marker to
    /// say, or `None` where it records nothing — which is a root nothing may be
    /// written into, the move included.
    pub(super) expected: Option<RootMarkerId>,
}
