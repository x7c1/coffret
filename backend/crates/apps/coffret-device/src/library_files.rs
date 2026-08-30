//! The three things a Library directory takes once its Master Key and its place
//! on Storage are settled.
//!
//! The catalog, the spool, and the settings file, in that order and for the same
//! reason in both flows: the settings file goes last, because a directory
//! carrying one is a Library anything may open.

use coffret_sqlite_index::SqliteIndex;

use crate::device_settings::DeviceSettings;
use crate::error::{CreationStep, Error, Result};
use crate::owner_only;
use crate::staging::Staging;

/// Writes the catalog, the spool and the settings into the staged directory.
pub(crate) fn write(staging: &Staging, settings: &DeviceSettings) -> Result<()> {
    // Created empty and owner-only first, then handed to SQLite: the catalog is
    // plaintext and names Entry Paths, so it must never exist at whatever mode
    // the process umask would have given it, not even for an instant.
    let index_file = staging.staged().index_file();
    owner_only::create_empty_file("creating the catalog", &index_file)
        .map_err(|cause| staging.failed(CreationStep::Index, cause))?;
    SqliteIndex::open(&index_file)
        .map_err(|cause| staging.failed(CreationStep::Index, Error::Index { cause }))?;

    owner_only::create_dir(
        "creating the spool directory",
        &staging.staged().spool_dir(),
    )
    .map_err(|cause| staging.failed(CreationStep::Spool, cause))?;

    // Last, because a directory carrying one is a Library anything may open.
    settings
        .write(staging.staged())
        .map_err(|cause| staging.failed(CreationStep::Settings, cause))
}
