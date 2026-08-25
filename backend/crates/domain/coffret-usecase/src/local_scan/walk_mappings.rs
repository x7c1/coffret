use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use coffret_model::EntryPath;
use tokio::fs;

use crate::device_state::Mapping;
use crate::local_error::LocalError;
use crate::local_mtime::mtime_of;
use crate::local_operation::LocalOperation;
use crate::local_scan::source_file::SourceFile;
use crate::scratch;

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
pub(crate) async fn walk_mappings(
    mappings: &[Mapping],
) -> Result<BTreeMap<EntryPath, SourceFile>, LocalError> {
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
                return Err(LocalError::PathCollision { path: held.path });
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
) -> Result<Vec<SourceFile>, LocalError> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];

    while let Some((directory, relative)) = stack.pop() {
        let mut listing = match fs::read_dir(&directory).await {
            Ok(listing) => listing,
            // A mapped root that is not there holds no files, and a directory
            // that went away mid-walk holds no more: neither is a reason to
            // fail a run over the folders that are there.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(cause) => return Err(LocalError::io(LocalOperation::Listing, directory, cause)),
        };

        while let Some(entry) = listing
            .next_entry()
            .await
            .map_err(|cause| LocalError::io(LocalOperation::Listing, directory.clone(), cause))?
        {
            let local_path = entry.path();
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(LocalError::UnrepresentableName { path: local_path });
            };
            // A temporary file a fetch was killed in the middle of writing. It
            // is coffret's own scratch and not user data, so it is passed over
            // rather than committed as an Entry (spec: EP-11).
            if scratch::is_scratch(name) {
                continue;
            }
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
                    return Err(LocalError::io(LocalOperation::Stating, local_path, cause))
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

#[cfg(test)]
mod tests {
    use super::*;

    // EP-11: a fetch writes its temporary file inside the very folder this walk
    // covers, so a run killed before the rename leaves one behind. Committing it
    // would put a partial file in the Library at an Entry Path the user never
    // asked for, which is why the two flows agree on a reserved prefix — and why
    // this is the one kind of name a scan passes over rather than reports
    // (spec: EP-1, EP-8).
    #[tokio::test]
    async fn a_temporary_file_a_fetch_left_is_not_a_source_file() {
        let root = tempfile::tempdir().expect("making a temporary directory must succeed");
        let container_id =
            coffret_format::generate_container_id().expect("the OS CSPRNG is available");
        let scratch_name = scratch::name(container_id);
        // A *folder* carrying the prefix, which no fetch makes but a user could.
        // The walk decides on the name before it stats what the name is, so the
        // whole subtree under it is passed over too — which is the width of the
        // trade EP-11 records.
        let scratch_folder = format!("{}album", scratch::PREFIX);

        for folder in ["below", &scratch_folder] {
            fs::create_dir_all(root.path().join(folder))
                .await
                .expect("making a folder must succeed");
        }
        for relative in [
            "a.jpg".to_owned(),
            "below/b.png".to_owned(),
            // At the top of the walk and below it, because the walk's other
            // reason to pass a name over applies only at the top (spec: EP-9).
            scratch_name.clone(),
            format!("below/{scratch_name}"),
            format!("{scratch_folder}/c.gif"),
        ] {
            fs::write(root.path().join(relative), b"some bytes")
                .await
                .expect("writing a file must succeed");
        }

        let found = walk_mappings(&[Mapping {
            prefix: None,
            local_root: root.path().to_path_buf(),
        }])
        .await
        .expect("walking a mapped folder must succeed");

        assert_eq!(
            found.keys().cloned().collect::<Vec<_>>(),
            vec![EntryPath::new("a.jpg"), EntryPath::new("below/b.png")],
            "the user's files, and nothing under a name carrying the reserved prefix",
        );
    }
}
