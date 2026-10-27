//! What `change-passphrase` answers with.

use serde::Serialize;

use coffret_device::LibraryOnDevice;

/// The Library whose Passphrase was changed on this device.
///
/// Nothing else: neither Passphrase is ever in an answer, and the Master Key
/// and its epoch are the ones the Library already had (spec: DK-6).
#[derive(Serialize)]
pub struct ChangedPassphrase {
    /// The Library's name, and null where this device knows none yet: never
    /// its ID in the name's place.
    library: Option<String>,
    library_id: String,
}

impl ChangedPassphrase {
    /// A change of `library`'s Passphrase on this device.
    pub fn new(library: LibraryOnDevice) -> Self {
        Self {
            library: library.name,
            library_id: library.library_id.to_hex(),
        }
    }
}
