//! The three things a Library directory takes once its Master Key and its place
//! on Storage are settled.
//!
//! The catalog, the spool, and the settings file, in that order and for the same
//! reason in both flows: the settings file goes last, because a directory
//! carrying one is a Library anything may open.

use crate::device_settings::DeviceSettings;
use crate::error::{CreationStep, Error, Result};
use crate::owner_only;
use crate::reach::Reach;
use crate::staging::Staging;

/// Writes the catalog, the spool and the settings into the staged directory.
pub(crate) fn write(staging: &Staging, settings: &DeviceSettings, reach: &Reach) -> Result<()> {
    // Owner-only from the moment it exists, which `SqliteIndex::open` sees to
    // for every caller rather than this one alone.
    reach
        .open_index(&staging.staged().index_file())
        .map_err(|cause| staging.failed(CreationStep::Index, Error::Index { cause }))?;

    owner_only::create_dir(&staging.staged().spool_dir())
        .map_err(|cause| staging.failed(CreationStep::Spool, cause))?;

    // Last, because a directory carrying one is a Library anything may open.
    settings
        .write(staging.staged())
        .map_err(|cause| staging.failed(CreationStep::Settings, cause))
}
