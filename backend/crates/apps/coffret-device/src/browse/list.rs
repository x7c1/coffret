use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, ContainerKind, EntryPath};
use tracing::{debug, warn};

use super::{ChildFolder, EntryState, FileRow, FolderListing};
use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// What one folder holds, one level down; `None` is the Library root.
    ///
    /// Three questions of the catalog and no more: the Entries under the folder,
    /// which of them this device has, and what kind of Container each lives in.
    /// A row's state comes from a present materialization record and from
    /// nothing else, because a mapping asserts nothing about what is on disk
    /// (spec: EP-9, EP-10).
    ///
    /// An Entry standing at exactly the folder's own path is not in the listing.
    /// The Library admits one — nothing stops a file and a folder sharing a
    /// path, since neither is a thing the Library has — and it is not a child of
    /// itself; whoever named the folder is who has it.
    pub async fn list(&self, folder: Option<&EntryPath>) -> Result<FolderListing> {
        let entries = self.index.entries_under(folder).await?;
        let present: BTreeSet<EntryPath> = self
            .index
            .present_under(folder)
            .await?
            .into_iter()
            .map(|local| local.observation.path)
            .collect();
        let kinds: BTreeMap<ContainerId, ContainerKind> = self
            .index
            .containers_under(folder)
            .await?
            .into_iter()
            .map(|container| (container.id, container.kind))
            .collect();

        let mut folders = Vec::new();
        let mut named: BTreeSet<&str> = BTreeSet::new();
        let mut files = Vec::new();
        for location in &entries {
            let Some(rest) = inside(folder, location.path()) else {
                // The Entry standing at the folder's own path.
                continue;
            };
            match rest.split_once('/') {
                // Anything with a separator left in it stands under a folder of
                // this one, and names that folder by what comes before it.
                Some((name, _)) => {
                    if named.insert(name) {
                        folders.push(ChildFolder {
                            name: name.to_owned(),
                            path: child_path(folder, name),
                        });
                    }
                }
                None => files.push(FileRow {
                    name: rest.to_owned(),
                    path: location.path().clone(),
                    size: location.entry.size,
                    mtime: location.entry.mtime,
                    state: match present.contains(location.path()) {
                        true => EntryState::Present,
                        false => EntryState::Remote,
                    },
                    container: kind_of(&kinds, location.container_id),
                }),
            }
        }

        debug!(
            operation = "list",
            library = %self.library_id,
            entries = entries.len(),
            folders = folders.len(),
            files = files.len(),
            "listed one folder of the Library",
        );
        Ok(FolderListing {
            path: folder.cloned(),
            folders,
            files,
        })
    }
}

/// What is left of an Entry Path once the folder above it is taken off, or
/// `None` where the path is the folder itself.
///
/// The separator is stripped along with the prefix, which is what keeps a
/// sibling whose name merely starts with the same letters out of the folder
/// (spec: EP-2, EP-9) — though the catalog's own range has already excluded it.
fn inside<'a>(folder: Option<&EntryPath>, path: &'a EntryPath) -> Option<&'a str> {
    match folder {
        None => Some(path.as_str()),
        Some(folder) => path
            .as_str()
            .strip_prefix(folder.as_str())
            .and_then(|rest| rest.strip_prefix('/')),
    }
}

/// Which kind of Container one Entry lives in (spec: PK-15).
///
/// The two reads behind the map are two reads, and another process may commit
/// between them: a Container that stopped being current after the Entries were
/// read is a Container this listing has an Entry for and no summary of. It is
/// rare, it is nobody's mistake, and the next listing will not have it — but it
/// is not nothing either, because what reads this field decides from it whether
/// an Entry can be replaced one file at a time. So it is reported, and the row
/// falls back to the answer that refuses nothing on its own.
fn kind_of(
    kinds: &BTreeMap<ContainerId, ContainerKind>,
    container_id: ContainerId,
) -> ContainerKind {
    match kinds.get(&container_id) {
        Some(kind) => *kind,
        None => {
            warn!(
                operation = "list",
                container = %container_id,
                "the catalog holds an Entry in a Container it summarizes no longer",
            );
            ContainerKind::OneFile
        }
    }
}

/// Where a child of `folder` called `name` stands in the Library.
fn child_path(folder: Option<&EntryPath>, name: &str) -> EntryPath {
    // Both halves are already the Library's spelling — one is an Entry Path and
    // the other a component cut out of one — so composing them changes nothing
    // (spec: EP-1).
    match folder {
        None => EntryPath::nfc(name),
        Some(folder) => EntryPath::nfc(format!("{}/{name}", folder.as_str())),
    }
}
