use coffret_device::LibraryOnDevice;
use serde::Serialize;

/// One Library on this device, as a Passphrase window lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListedLibrary {
    /// The Library's ID, in the 16 lowercase hex characters it is spelled in
    /// (spec: FM-18): what a window submits to say which Library to open.
    pub id: String,
    /// The Library's name, and `None` where nothing has named it yet: the
    /// window then says it has none and shows its ID as its ID, rather than
    /// the ID where the name goes.
    pub name: Option<String>,
    /// Whether more than one directory on this device holds it: it is listed
    /// once, and the window marks it, since opening it is refused until one
    /// of them is gone.
    pub needs_attention: bool,
}

impl From<LibraryOnDevice> for ListedLibrary {
    fn from(library: LibraryOnDevice) -> Self {
        Self {
            id: library.library_id.to_hex(),
            name: library.name,
            needs_attention: library.needs_attention,
        }
    }
}
