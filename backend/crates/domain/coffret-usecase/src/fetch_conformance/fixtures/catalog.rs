use std::path::Path;

use coffret_model::{EntryLocation, EntryPath};

use crate::device_state::Mapping;
use crate::index::Index;

/// Maps a device's folder onto the Library at `prefix` (spec: EP-9).
pub(crate) async fn map(index: &dyn Index, prefix: Option<&str>, local_root: &Path) {
    index
        .set_mapping(Mapping {
            prefix: prefix.map(EntryPath::new),
            local_root: local_root.to_path_buf(),
        })
        .await
        .expect("recording a mapping must succeed");
}

/// Where the current Entry at one path lives, which the case expects to exist.
pub(crate) async fn entry_at(index: &dyn Index, path: &str) -> EntryLocation {
    index
        .entry_at(&EntryPath::new(path))
        .await
        .expect("asking a catalog for a path must succeed")
        .unwrap_or_else(|| panic!("{path:?} must be a current Entry"))
}
