use crate::device_state::RootMarkerId;
use crate::local_scan::SourceFile;

/// A file this device materialized whose Entry has left the Library, and which
/// still holds what this device last made it match (spec: EP-15).
///
/// What a sync discards into the trash once its scan is over, and what a
/// deletion discards right after its commit. The identity the file's mapping
/// expects of its root travels with it, because the discard is a write into the
/// mapped folder and is made only where the root proves to be the one the
/// mapping was recorded against (spec: EP-13).
#[derive(Debug, Clone)]
pub(crate) struct Departed {
    /// The file to discard.
    pub(crate) source: SourceFile,
    /// What the mapping the file was found under expects its root's marker to
    /// say, or `None` where it records nothing — which is a root nothing may be
    /// written into, the discard included.
    pub(crate) expected: Option<RootMarkerId>,
}
