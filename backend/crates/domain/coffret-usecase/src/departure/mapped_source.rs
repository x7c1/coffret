use coffret_model::EntryPath;

use crate::device_state::Mapping;
use crate::fetch::relative_below;
use crate::folder_entry_kind::FolderEntryKind;
use crate::local_io_error::LocalIoError;
use crate::local_scan::SourceFile;
use crate::mapped_relative_location::MappedRelativeLocation;
use crate::mapped_roots::MappedRoots;
use crate::root_marker;
use crate::scratch;
use crate::trash_folders;

/// The file standing for `path` under `mapping`, as a walk of the mapped
/// folder would have found it, or `None` where the walk would have found none
/// (spec: EP-8, EP-9).
///
/// What a deletion asks of each Entry it deleted: it knows the Entry Path and
/// not the file, so instead of walking the whole folder it reads only the
/// folders on the way down to the one file. Each folder is listed through the
/// same capability the walk lists it through, with links unfollowed, and each
/// name is read the way the walk reads it — into NFC before it is compared
/// (spec: EP-1) — so the [`SourceFile`] that comes back carries the length and
/// modification time the walk would have, and the location below the root
/// spelled the way the folder spells it, which is what a move to the trash is
/// asked for (spec: EP-8).
///
/// `None` for everything the walk would not have found: nothing at the path, a
/// folder or a link there rather than a regular file, a folder on the way down
/// that is not one, a path that is exactly the mapping's own prefix, and a path
/// through what the walk steps over — the management area, a fetch's scratch,
/// or a desktop's trash folder at the top of the root (spec: EP-11, EP-14,
/// EP-16). A folder that is not there is absence, as it is everywhere this
/// capability answers (spec: OC-8); only a refusal to list one is an error.
pub(crate) async fn mapped_source(
    roots: &dyn MappedRoots,
    mapping: &Mapping,
    path: &EntryPath,
) -> Result<Option<SourceFile>, LocalIoError> {
    let Some(relative) = relative_below(mapping, path) else {
        return Ok(None);
    };
    let wanted: Vec<&str> = relative.as_str().split('/').collect();
    if wanted
        .first()
        .is_some_and(|top| trash_folders::is_trash_folder(top))
        || wanted
            .iter()
            .any(|name| root_marker::is_management_area(name) || scratch::is_scratch(name))
    {
        return Ok(None);
    }

    let root = mapping.local_root.as_path();
    let mut below: Option<MappedRelativeLocation> = None;
    for (depth, component) in wanted.iter().enumerate() {
        let last = depth + 1 == wanted.len();
        let Some(entries) = roots.list_folder(root, below.as_ref()).await? else {
            return Ok(None);
        };
        // The name the folder spells this component with, read as the walk
        // reads one: a name that is not text, or that no Entry Path can be made
        // of, is not this component however it is spelled.
        let Some(entry) = entries.into_iter().find(|entry| {
            entry
                .name
                .to_str()
                .and_then(|text| EntryPath::parse(text).ok())
                .is_some_and(|name| name.as_str() == *component)
        }) else {
            return Ok(None);
        };
        let location = match &below {
            None => MappedRelativeLocation::from_component(entry.name.clone()),
            Some(folder) => folder.below_component(entry.name.clone()),
        };
        match (entry.kind, last) {
            (FolderEntryKind::Folder, false) => below = Some(location),
            (FolderEntryKind::File { size, mtime, btime }, true) => {
                return Ok(Some(SourceFile {
                    path: path.clone(),
                    root: mapping.local_root.clone(),
                    relative: location,
                    size,
                    mtime,
                    btime,
                }))
            }
            // A file where a folder was due, a folder where the file was, or a
            // link or anything else that is neither: not followed, and not the
            // file the Entry was materialized as (spec: EP-8).
            _ => return Ok(None),
        }
    }
    Ok(None)
}
