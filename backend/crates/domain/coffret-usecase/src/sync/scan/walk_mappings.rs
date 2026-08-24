use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

use coffret_model::{EntryPath, Mtime};
use tokio::fs;

use crate::device_state::Mapping;
use crate::sync::source_file::SourceFile;
use crate::sync::sync_error::{LocalOperation, SyncError, SyncResult};

/// Every regular file under every mapping, by the Entry Path it stands at.
///
/// A device may map a local root to the Library root and other local roots to
/// top-level components, and then the top-level mapping represents its subtree
/// while the Library-root mapping represents *the remainder* (spec: EP-9). So
/// the root walk stops at every top-level name another mapping stands for: a
/// folder called `albums` under the root-mapped folder is not a second spelling
/// of the `albums/` subtree, and walking it would either claim Entry Paths the
/// other mapping owns or collide with the files it holds.
///
/// Two local files reaching one Entry Path is then refused rather than
/// resolved: choosing one of them would back up whichever the walk happened to
/// reach second, and renaming one would invent a Library position the user never
/// asked for (spec: EP-4).
pub(super) async fn walk_mappings(
    mappings: &[Mapping],
) -> SyncResult<BTreeMap<EntryPath, SourceFile>> {
    let claimed: BTreeSet<&str> = mappings
        .iter()
        .filter_map(|mapping| mapping.prefix.as_ref())
        .map(EntryPath::as_str)
        .collect();

    let mut found: BTreeMap<EntryPath, SourceFile> = BTreeMap::new();
    for mapping in mappings {
        // A top-level mapping stands for the whole of its own subtree, so
        // nothing is held back from its walk.
        let elsewhere = match mapping.prefix {
            Some(_) => BTreeSet::new(),
            None => claimed.clone(),
        };
        for source in walk(&mapping.local_root, mapping.prefix.as_ref(), &elsewhere).await? {
            if let Some(held) = found.insert(source.path.clone(), source) {
                return Err(SyncError::PathCollision { path: held.path });
            }
        }
    }
    Ok(found)
}

/// Every regular file under one local root, at the Entry Paths the mapping
/// gives them (spec: EP-9).
///
/// Regular files only, and symbolic links are neither followed nor given an
/// Entry Path of their own — which is why every entry is stated with
/// `symlink_metadata` rather than `metadata` (spec: EP-8).
///
/// `elsewhere` names the top-level components another mapping represents, which
/// this walk therefore does not enter (spec: EP-9). It is a top-level name and
/// so an Entry Path component, which is why nothing here says which one was
/// passed over.
async fn walk(
    root: &Path,
    prefix: Option<&EntryPath>,
    elsewhere: &BTreeSet<&str>,
) -> SyncResult<Vec<SourceFile>> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];

    while let Some((directory, relative)) = stack.pop() {
        let mut listing = match fs::read_dir(&directory).await {
            Ok(listing) => listing,
            // A mapped root that is not there holds no files, and a directory
            // that went away mid-walk holds no more: neither is a reason to
            // fail a run over the folders that are there.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(cause) => {
                return Err(SyncError::Io {
                    operation: LocalOperation::Listing,
                    path: directory,
                    cause,
                })
            }
        };

        while let Some(entry) = listing.next_entry().await.map_err(|cause| SyncError::Io {
            operation: LocalOperation::Listing,
            path: directory.clone(),
            cause,
        })? {
            let local_path = entry.path();
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(SyncError::UnrepresentableName { path: local_path });
            };
            // At the top of this walk the name *is* the top-level component, so
            // this is where a subtree another mapping represents is left to it
            // (spec: EP-9).
            if relative.is_empty() && elsewhere.contains(name) {
                continue;
            }
            let below = if relative.is_empty() {
                name.to_owned()
            } else {
                format!("{relative}/{name}")
            };

            let metadata = match fs::symlink_metadata(&local_path).await {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(cause) => {
                    return Err(SyncError::Io {
                        operation: LocalOperation::Stating,
                        path: local_path,
                        cause,
                    })
                }
            };
            if metadata.is_dir() {
                stack.push((local_path, below));
            } else if metadata.is_file() {
                found.push(SourceFile {
                    path: entry_path(prefix, &below),
                    local_path,
                    size: metadata.len(),
                    mtime: mtime_of(&metadata),
                });
            }
        }
    }
    Ok(found)
}

/// Where a file sits in the Library, given the mapping it was found under
/// (spec: EP-9).
fn entry_path(prefix: Option<&EntryPath>, relative: &str) -> EntryPath {
    match prefix {
        Some(prefix) => EntryPath::new(format!("{}/{relative}", prefix.as_str())),
        None => EntryPath::new(relative),
    }
}

/// A file's modification time, as the value an Entry carries (spec: FM-9).
///
/// A clock that reports a moment before 1970 is recorded as one rather than
/// clamped, for the reason [`Mtime`] admits those at all: refusing the value
/// would lose the file's own time instead of correcting it. A filesystem that
/// keeps no modification time at all leaves the epoch, which is the only answer
/// available and is not evidence about the file.
fn mtime_of(metadata: &Metadata) -> Mtime {
    let Ok(modified) = metadata.modified() else {
        return Mtime::from_unix_seconds(0);
    };
    Mtime::from_unix_seconds(match modified.duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs())
            .map(|seconds| -seconds)
            .unwrap_or(i64::MIN),
    })
}
