//! What `change-passphrase` answers with.

use serde::Serialize;

/// The Library whose Passphrase was changed on this device.
///
/// Nothing else: neither Passphrase is ever in an answer, and the Master Key
/// and its epoch are the ones the Library already had (spec: DK-6).
#[derive(Serialize)]
pub struct ChangedPassphrase {
    library: String,
}

impl ChangedPassphrase {
    /// A change of `library`'s Passphrase on this device.
    pub fn new(library: String) -> Self {
        Self { library }
    }
}
