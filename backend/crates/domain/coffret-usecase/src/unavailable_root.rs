use std::path::PathBuf;

use coffret_model::EntryPath;

/// A mapping whose local root says nothing about the Library (spec: EP-12).
///
/// A mapped root that is not there, and one that is there and empty while
/// standing on a filesystem the mapping does not record, are both what an
/// unplugged disk or a lost network mount leaves behind — and neither is a
/// folder the user emptied. So nothing under such a root is walked, no Entry
/// under it is reported as deleted locally, no file under it is selected for
/// `update` or `freeze`, and the run reports the mapping instead of returning
/// silently: a successful run carrying one of these has scanned less than the
/// device's mappings cover (spec: PK-14).
///
/// The local root travels in the value because the caller is what decides what
/// to do about it. It never travels into a log line, and neither does the
/// prefix — an Entry Path component is no more loggable than a local path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableRoot {
    /// The top-level component the mapping stands for, or `None` for the
    /// Library root.
    pub prefix: Option<EntryPath>,
    /// The folder on this device the mapping names.
    pub local_root: PathBuf,
    /// What made it unavailable.
    pub reason: RootUnavailable,
}

/// Why nothing under a mapped root may be read as evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootUnavailable {
    /// The root directory is not there.
    Missing,
    /// The root is there and empty, and stands on a filesystem the mapping does
    /// not record — what an unplugged disk or an unmounted share leaves behind.
    AnotherFilesystem,
}
