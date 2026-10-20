use std::path::PathBuf;

use coffret_explorer_host::PageSource;

/// The variable a development run names the explorer's build output on disk
/// with, so that the host serves that directory in place of the page compiled
/// in, and a rebuilt page is served on the next reload.
///
/// For a development shell in front of a checkout, and read only where it is
/// set to something: a shipped app serves the page it carries.
pub const PAGE_DIR: &str = "COFFRET_DESKTOP_PAGE_DIR";

/// Where the explorer's page is served from: the page compiled in
/// (`embed-web`), unless [`PAGE_DIR`] names a directory in its place.
pub(crate) fn page_source() -> PageSource {
    match std::env::var_os(PAGE_DIR).filter(|value| !value.is_empty()) {
        Some(directory) => {
            tracing::info!(
                kind = "page_source",
                source = "directory",
                "the explorer's page is served from a directory on disk, as {PAGE_DIR} asks",
            );
            PageSource::Directory(PathBuf::from(directory))
        }
        None => PageSource::Embedded,
    }
}
