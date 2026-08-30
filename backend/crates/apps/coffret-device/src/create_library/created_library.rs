use std::path::PathBuf;

use coffret_format::RecoveryCode;

use crate::device_settings::DeviceSettings;

/// What a Library that has just been created hands back.
///
/// The Recovery Code is the reason this is a value rather than a `()`: it is
/// the only copy of the Master Key that exists outside this device, and nothing
/// stores it — writing it out again takes a device that still holds the Library
/// and the Passphrase that unlocks its stored Master Key. Losing every device
/// that holds the Library and every Recovery Code written down for it makes the
/// Library unreadable by anyone, coffret included.
#[derive(Debug)]
pub struct CreatedLibrary {
    /// The Master Key and its epoch, in the form a person writes down
    /// (spec: KD-11).
    pub recovery_code: RecoveryCode,
    /// What the device recorded about where the Library lives.
    pub settings: DeviceSettings,
    /// The directory the Library now occupies on this device.
    pub path: PathBuf,
}
