use coffret_model::ContainerId;

use crate::fetch::unheld_reason::UnheldReason;

/// A parcel this device's record said it held, and that it did not.
///
/// The file was gone, or its ciphertext no longer authenticated as that parcel
/// of that Container (spec: FM-5, FM-7, FM-8). Either way the parcel is not
/// held: it is let go and asked of Storage again (spec: PK-21). It
/// is said rather than silently read again because that second read is one
/// the provider observes, and because a kept file that changed under the
/// device is a disk worth hearing about.
///
/// The Container and the parcel's position and nothing else: neither names an
/// Entry, and the local path stays out of anything that might be logged
/// (spec: EL-1).
#[derive(Debug, Clone)]
pub struct UnheldParcel {
    /// The Container the parcel is a part of.
    pub container_id: ContainerId,
    /// Which parcel it was, counted from the first chunk (spec: PK-19).
    pub index: u64,
    /// What was wrong with the kept file.
    pub reason: UnheldReason,
}
