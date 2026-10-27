//! What `recovery-code` answers with.

use serde::Serialize;

use coffret_device::LibraryOnDevice;

/// What `recovery-code` printed again, and the Library it is the code of.
#[derive(Serialize)]
pub struct RecoveryCode {
    /// The Library's name, and null where this device knows none yet: never
    /// its ID in the name's place.
    library: Option<String>,
    library_id: String,
    recovery_code: String,
}

impl RecoveryCode {
    /// `code`, as `library`'s Recovery Code.
    pub fn new(library: LibraryOnDevice, code: &coffret_device::RecoveryCode) -> Self {
        Self {
            library: library.name,
            library_id: library.library_id.to_hex(),
            recovery_code: code.to_grouped_string(),
        }
    }
}
