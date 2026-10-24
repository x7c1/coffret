use std::path::PathBuf;

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
use crate::system_files;
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
/// spelled the way the folder spells it, which is what a discard is
/// asked for (spec: EP-8).
///
/// `None` for everything the walk would not have found: nothing at the path, a
/// folder or a link there rather than a regular file, a folder on the way down
/// that is not one, a path that is exactly the mapping's own prefix, and a path
/// through what the walk steps over — the management area, a fetch's scratch,
/// a desktop's trash folder at the top of the root, or a name an operating
/// system keeps (spec: EP-11, EP-14, EP-16, EP-17). A folder that is not there
/// is absence, as it is everywhere this capability answers (spec: OC-8); only a
/// refusal to list one is an error.
///
/// The last is asked here rather than in [`mapped_file`], which a relocation
/// asks of the path a file stands at as well: an Entry carried in at such a
/// path before EP-17 is deleted without the operating system's file standing
/// there being discarded with it, since nobody handed that file to the Library.
pub(crate) async fn mapped_source(
    roots: &dyn MappedRoots,
    mapping: &Mapping,
    path: &EntryPath,
) -> Result<Option<SourceFile>, LocalIoError> {
    let kept_by_system = relative_below(mapping, path).is_some_and(|relative| {
        system_files::system_component(relative.as_str().split('/')).is_some()
    });
    if kept_by_system {
        return Ok(None);
    }
    Ok(match mapped_file(roots, mapping, path).await? {
        MappedFile::File(source) => Some(source),
        MappedFile::Nothing | MappedFile::Folder | MappedFile::Fenced { .. } => None,
    })
}

/// What stands for `path` under `mapping`, read the way
/// [`mapped_source`] reads it, with what the walk would not have found told
/// apart: nothing at all, a folder, and a name on the way — or at the path
/// itself — that is not a folder of the root (spec: EP-8).
///
/// What a relocation asks of both the path a file stands at and the path it is
/// to go to: a symbolic link on the way to either is a fence it is refused at,
/// and a folder standing where it would go is a destination already taken
/// (spec: EP-10).
pub(crate) async fn mapped_file(
    roots: &dyn MappedRoots,
    mapping: &Mapping,
    path: &EntryPath,
) -> Result<MappedFile, LocalIoError> {
    let Some(relative) = relative_below(mapping, path) else {
        return Ok(MappedFile::Nothing);
    };
    let wanted: Vec<&str> = relative.as_str().split('/').collect();
    if wanted
        .first()
        .is_some_and(|top| trash_folders::is_trash_folder(top))
        || wanted
            .iter()
            .any(|name| root_marker::is_management_area(name) || scratch::is_scratch(name))
    {
        return Ok(MappedFile::Nothing);
    }

    let root = mapping.local_root.as_path();
    let mut below: Option<MappedRelativeLocation> = None;
    for (depth, component) in wanted.iter().enumerate() {
        let last = depth + 1 == wanted.len();
        let Some(entries) = roots.list_folder(root, below.as_ref()).await? else {
            return Ok(MappedFile::Nothing);
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
            return Ok(MappedFile::Nothing);
        };
        let location = match &below {
            None => MappedRelativeLocation::from_component(entry.name.clone()),
            Some(folder) => folder.below_component(entry.name.clone()),
        };
        match (entry.kind, last) {
            (FolderEntryKind::Folder, false) => below = Some(location),
            (FolderEntryKind::File { size, mtime, btime }, true) => {
                return Ok(MappedFile::File(SourceFile {
                    path: path.clone(),
                    root: mapping.local_root.clone(),
                    relative: location,
                    size,
                    mtime,
                    btime,
                }))
            }
            (FolderEntryKind::Folder, true) => return Ok(MappedFile::Folder),
            // A file where a folder was due, or a link or anything else that is
            // neither: not followed, and not the file the Entry was
            // materialized as (spec: EP-8).
            _ => {
                return Ok(MappedFile::Fenced {
                    stopped_at: root.join(location.to_path_buf()),
                })
            }
        }
    }
    Ok(MappedFile::Nothing)
}

/// What stands for one Entry Path under a mapping, as [`mapped_file`] reads
/// it.
#[derive(Debug)]
pub(crate) enum MappedFile {
    /// A regular file, as the walk would have found it.
    File(SourceFile),
    /// Nothing: no name at the path or at a folder on the way to it, or a path
    /// through what the walk steps over.
    Nothing,
    /// A folder, standing where a file would.
    Folder,
    /// A name at the path or on the way to it that is not a folder of the
    /// root: a symbolic link, or a file where a folder is due.
    Fenced {
        /// The local path of that name.
        stopped_at: PathBuf,
    },
}
