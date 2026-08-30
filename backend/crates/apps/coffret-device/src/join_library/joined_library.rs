use std::path::PathBuf;

use crate::device_settings::DeviceSettings;

/// What a Library this device has just joined hands back.
///
/// No Recovery Code, which is the whole difference from
/// [`CreatedLibrary`](crate::CreatedLibrary): the code went in rather than
/// coming out, and the device that entered it already has it written down.
/// Asking for it again is [`recovery_code`](crate::recovery_code), which any
/// device holding the Library can answer.
#[derive(Debug)]
pub struct JoinedLibrary {
    /// What the device recorded about where the Library lives.
    pub settings: DeviceSettings,
    /// The directory the Library now occupies on this device.
    pub path: PathBuf,
}
