use std::path::PathBuf;

use coffret_usecase::device_state::Mapping;

use crate::marker_record::MarkerRecord;

/// What recording a mapping did (spec: EP-9, EP-13).
///
/// Two facts a caller has to be able to tell somebody, and neither is the
/// other's. What the mapping replaced is about the Library: moving a prefix takes
/// everything under the old root out of the Library's reach on this device, so a
/// caller that cannot say what was there cannot say what just happened. What
/// became of the marker is about the folder: it now carries an identity, and
/// whether that identity was written, adopted from a marker already standing
/// there, or issued in place of one is what tells a person whether this root is
/// the one they registered before.
#[derive(Debug)]
pub struct RecordedMapping {
    /// The root as it was recorded: absolute, with no symbolic link left in it.
    ///
    /// Not necessarily the path a caller handed over, which may have been
    /// relative or have passed through a link; a caller saying where the
    /// mapping now points says this one, so that its sentence and the
    /// mapping's record never stand in two forms of one folder.
    pub local_root: PathBuf,
    /// The mapping this one replaced, where the prefix was already mapped.
    pub replaced: Option<Mapping>,
    /// What became of the marker in the root that was recorded.
    pub marker: MarkerRecord,
}
