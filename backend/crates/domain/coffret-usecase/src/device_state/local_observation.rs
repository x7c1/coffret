use coffret_model::{ContentHash, EntryPath, Mtime};

use crate::device_state::device_time::DeviceTime;

/// What this device last saw of the local file at one Entry Path.
///
/// A scan compares this against what it finds on disk to decide whether a file
/// changed, so it records the two things a filesystem answers cheaply — length
/// and modification time — alongside when the device looked. It is not evidence
/// about the Library: the Entry's own size, mtime, and content hash live in the
/// Container that holds it (spec: FM-9), and this is only the local file the
/// device put there.
///
/// The one fact about the Entry it does keep is the content hash the file was
/// made to match, because it is the one a device cannot ask the Library for once
/// the Entry is gone: a catch-up that removes the Entry removes its hash with it,
/// and the device still has to tell a file left exactly as the Library had it
/// from one somebody edited since (spec: EP-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalObservation {
    /// The Library position the local file stands at.
    pub path: EntryPath,
    /// The file's length in bytes when the device last looked.
    pub size: u64,
    /// The file's modification time when the device last looked.
    pub mtime: Mtime,
    /// When the device last looked.
    pub at: DeviceTime,
    /// The content hash of the Entry this device last made the file match —
    /// by uploading it, fetching it, or finding it unchanged against it
    /// (spec: EP-10, EP-15).
    ///
    /// Every writer knows it and records it. `None` is only ever read back, off
    /// a row a build that did not record it wrote, and a reader takes it to
    /// mean that the content cannot be vouched for.
    pub hash: Option<ContentHash>,
}
