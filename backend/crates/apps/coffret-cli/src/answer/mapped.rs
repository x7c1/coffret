//! What `map` answers with.

use std::path::Path;

use serde::Serialize;

use coffret_device::{Mapping, MarkerRecord};

use super::said_path;

/// What `map` recorded.
///
/// The two folders are the ones the text form names on standard error.
#[derive(Serialize)]
pub struct Mapped {
    prefix: Option<String>,
    local_root: String,
    replaced: Option<String>,
    marker: &'static str,
}

impl Mapped {
    /// `prefix` recorded at `local_root`, over what it `replaced`.
    pub fn new(
        prefix: Option<String>,
        local_root: &Path,
        replaced: Option<&Mapping>,
        marker: &MarkerRecord,
    ) -> Self {
        Self {
            prefix,
            local_root: said_path(local_root),
            replaced: replaced.map(|mapping| said_path(&mapping.local_root)),
            marker: match marker {
                MarkerRecord::Written => "written",
                MarkerRecord::Adopted => "adopted",
                MarkerRecord::Reset => "reset",
            },
        }
    }
}
