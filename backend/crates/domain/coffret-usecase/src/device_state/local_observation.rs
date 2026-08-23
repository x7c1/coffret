use coffret_model::{EntryPath, Mtime};

use crate::device_state::device_time::DeviceTime;

/// What this device last saw of the local file at one Entry Path.
///
/// A scan compares this against what it finds on disk to decide whether a file
/// changed, so it records the two things a filesystem answers cheaply — length
/// and modification time — alongside when the device looked. It is not evidence
/// about the Library: the Entry's own size, mtime, and content hash live in the
/// Container that holds it (spec: FM-9), and this is only the local file the
/// device put there.
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
}
