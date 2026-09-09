use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use coffret_model::EntryPath;

/// A validated relative location below a configured mapped root.
///
/// Unlike an [`EntryPath`], this preserves the filesystem's original spelling.
/// A decomposed local name may normalize to a composed Library path, but the
/// reader must still reopen the name that the directory actually contained.
/// Construction only accepts already validated Entry Paths or components a
/// scan has successfully interpreted as one, so `.` and `..` can never reach a
/// descriptor descent through this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedRelativeLocation {
    components: Vec<OsString>,
}

impl MappedRelativeLocation {
    /// The local spelling of a canonical Entry Path.
    pub fn from_entry_path(path: &EntryPath) -> Self {
        Self {
            components: path.as_str().split('/').map(OsString::from).collect(),
        }
    }

    /// One filesystem component after the scan has validated its text.
    pub(crate) fn from_component(component: OsString) -> Self {
        Self {
            components: vec![component],
        }
    }

    /// Appends one filesystem component after the scan has validated its text.
    pub(crate) fn below_component(&self, component: OsString) -> Self {
        let mut components = self.components.clone();
        components.push(component);
        Self { components }
    }

    /// The component spellings, in descent order.
    pub fn components(&self) -> impl Iterator<Item = &OsStr> {
        self.components.iter().map(OsString::as_os_str)
    }

    /// The validated text of each component.
    pub fn text_components(&self) -> impl Iterator<Item = &str> {
        self.components.iter().map(|component| {
            component
                .to_str()
                .expect("a mapped relative location contains only validated Unicode components")
        })
    }

    /// The relative path used for diagnostics, collision checks, and in-memory emulation.
    pub fn to_path_buf(&self) -> PathBuf {
        self.components.iter().collect()
    }
}
