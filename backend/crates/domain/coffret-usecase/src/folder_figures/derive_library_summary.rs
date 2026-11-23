use super::{present_paths, LibrarySummary};
use crate::index::Index;
use crate::index_error::IndexResult;

/// What the whole Library comes to, read off every Entry, every present
/// record and every Container.
pub async fn derive_library_summary(index: &(impl Index + ?Sized)) -> IndexResult<LibrarySummary> {
    let entries = index.entries_under(None).await?;
    let present = present_paths(index, None).await?;
    let containers = index.containers_under(None).await?.len() as u64;
    let mut summary = LibrarySummary {
        containers,
        ..LibrarySummary::default()
    };
    for location in &entries {
        let size = location.entry.extent.size();
        summary.files_total += 1;
        summary.size += size;
        if present.contains(location.path()) {
            summary.files_present += 1;
            summary.size_present += size;
        }
        summary.mtime = summary.mtime.max(Some(location.entry.mtime));
    }
    Ok(summary)
}
