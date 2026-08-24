use std::collections::{BTreeMap, BTreeSet};

use coffret_model::EntryPath;

use crate::device_state::Mapping;
use crate::index::Index;
use crate::sync::deferred::Deferred;
use crate::sync::source_file::SourceFile;
use crate::sync::sync_error::SyncResult;

/// The files this device materialized that are no longer on disk (spec: EP-10).
///
/// Only rows the port reports under a mapped prefix are considered, so a
/// subtree this device does not map is not a subtree of deletions. A row whose
/// path no walk found is a file the device put there and lost; one path can be
/// reported by two mappings, so the findings are deduplicated by path.
pub(super) async fn deletions(
    index: &dyn Index,
    mappings: &[Mapping],
    found: &BTreeMap<EntryPath, SourceFile>,
) -> SyncResult<Vec<Deferred>> {
    let mut gone = BTreeSet::new();
    for mapping in mappings {
        for local in index.present_under(mapping.prefix.as_ref()).await? {
            if !found.contains_key(&local.observation.path) {
                gone.insert(local.observation.path);
            }
        }
    }
    Ok(gone
        .into_iter()
        .map(|path| Deferred::DeletedLocally { path })
        .collect())
}
