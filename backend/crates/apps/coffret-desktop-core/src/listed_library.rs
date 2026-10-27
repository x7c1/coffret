use coffret_device::LibraryOnDevice;
use serde::Serialize;

/// One Library on this device, as a Passphrase window lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListedLibrary {
    /// The Library's ID, in the 16 lowercase hex characters it is spelled in
    /// (spec: FM-18): what a window submits to say which Library to open.
    pub id: String,
    /// What the window shows: the Library's name, or its ID where nothing has
    /// named it yet.
    pub name: String,
}

impl From<LibraryOnDevice> for ListedLibrary {
    fn from(library: LibraryOnDevice) -> Self {
        let id = library.library_id.to_hex();
        Self {
            name: library.name.unwrap_or_else(|| id.clone()),
            id,
        }
    }
}
