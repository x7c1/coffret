use std::collections::BTreeMap;

use coffret_model::{ContainerId, ContainerKind, EntryPath};

use super::{present_paths, FolderContents, FolderFile, FolderSummary};
use crate::index::Index;
use crate::index_error::IndexResult;

/// One folder's contents, read off the whole subtree under it.
///
/// Three reads — the Entries under the folder, which of them this device has,
/// and the Containers they live in — and a pass over the subtree that counts
/// each child folder's Entries at every depth.
pub async fn derive_folder_contents(
    index: &(impl Index + ?Sized),
    folder: Option<&EntryPath>,
) -> IndexResult<FolderContents> {
    let entries = index.entries_under(folder).await?;
    let present = present_paths(index, folder).await?;
    let kinds: BTreeMap<ContainerId, ContainerKind> = index
        .containers_under(folder)
        .await?
        .into_iter()
        .map(|container| (container.id, container.kind))
        .collect();

    let mut folders: BTreeMap<EntryPath, FolderSummary> = BTreeMap::new();
    let mut files = Vec::new();
    for location in entries {
        let path = location.path().clone();
        let here = present.contains(&path);
        if path.parent().as_ref() == folder {
            files.push(FolderFile {
                container: kinds.get(&location.container_id).copied(),
                location,
                present: here,
            });
            continue;
        }
        // Anything deeper stands under a child of this folder, which is a
        // prefix of the Entry's own path (spec: EP-2). The Entry at exactly
        // the folder's own path stands under none.
        let Some(child) = child_holding(folder, &path) else {
            continue;
        };
        let size = location.entry.extent.size();
        let mtime = location.entry.mtime;
        let summary = folders
            .entry(child.clone())
            .or_insert_with(|| FolderSummary {
                path: child,
                files_total: 0,
                files_present: 0,
                size: 0,
                mtime,
            });
        summary.files_total += 1;
        summary.files_present += u64::from(here);
        summary.size += size;
        summary.mtime = summary.mtime.max(mtime);
    }
    Ok(FolderContents {
        folders: folders.into_values().collect(),
        files,
    })
}

/// The child of `folder` that `path` stands under, or `None` where `path` does
/// not lie strictly beneath `folder`.
fn child_holding(folder: Option<&EntryPath>, path: &EntryPath) -> Option<EntryPath> {
    let mut child = path.parent()?;
    loop {
        let above = child.parent();
        if above.as_ref() == folder {
            return Some(child);
        }
        child = above?;
    }
}
