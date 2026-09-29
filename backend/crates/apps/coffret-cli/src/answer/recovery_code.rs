//! What `recovery-code` answers with.

use serde::Serialize;

/// What `recovery-code` printed again.
#[derive(Serialize)]
pub struct RecoveryCode {
    library: String,
    recovery_code: String,
}

impl RecoveryCode {
    /// `code`, as `library`'s Recovery Code.
    pub fn new(library: String, code: &coffret_device::RecoveryCode) -> Self {
        Self {
            library,
            recovery_code: code.to_grouped_string(),
        }
    }
}
