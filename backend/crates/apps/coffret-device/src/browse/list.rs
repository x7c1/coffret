use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, ContainerKind, EntryPath};
use coffret_usecase::device_state::Mapping;
use tracing::{debug, warn};

use super::{ChildFolder, EntryState, FileRow, FolderListing};
use crate::error::Result;
use crate::folder_paths::{child_path, inside};
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// What one folder holds, one level down; `None` is the Library root.
    ///
    /// Four questions of the catalog and no more: the Entries under the folder,
    /// which of them this device has, what kind of Container each lives in, and
    /// which parts of the Library this device has a folder for at all. A row's
    /// state comes from a present materialization record and from nothing else,
    /// because a mapping asserts nothing about what is on disk (spec: EP-9,
    /// EP-10) — the mappings answer the separate question of whether anything
    /// here could be fetched, which is [`mapped`](FolderListing::mapped).
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
        let reach = Reach::of(self.index.mappings().await?);

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
                        let path = child_path(folder, name);
                        folders.push(ChildFolder {
                            name: name.to_owned(),
                            mapped: reach.reaches(Some(&path)),
                            path,
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
            mapped: reach.reaches(folder),
            path: folder.cloned(),
            folders,
            files,
        })
    }
}

/// Which parts of the Library this device has a folder for (spec: EP-9).
///
/// The mappings partition the Library's namespace between them: a top-level
/// mapping represents its own subtree, and a Library-root mapping represents
/// everything the top-level ones do not. So whether one folder is reachable is
/// decided by its top-level component alone, and this is the two facts about
/// the mappings that decide it.
struct Reach {
    /// Whether a mapping stands at the Library root. With one present nothing
    /// is out of reach, since it represents whatever no other mapping claims.
    root: bool,
    /// The top-level components a mapping of their own claims.
    claimed: BTreeSet<String>,
}

impl Reach {
    /// What this device's mappings come to, as the question a listing asks.
    fn of(mappings: Vec<Mapping>) -> Self {
        let mut root = false;
        let mut claimed = BTreeSet::new();
        for mapping in mappings {
            match mapping.prefix {
                None => root = true,
                Some(prefix) => {
                    claimed.insert(prefix.as_str().to_owned());
                }
            }
        }
        Self { root, claimed }
    }

    /// Whether a mapping reaches one folder, `None` being the Library root.
    ///
    /// The root is reached by the root mapping alone: a top-level mapping
    /// stands for its own subtree and not for what sits beside it, so a device
    /// that maps only `albums` has nowhere to put a file that lives at the top
    /// of the Library. Every other folder is reached by whichever mapping
    /// claims its top-level component, or by the root mapping where none does.
    fn reaches(&self, folder: Option<&EntryPath>) -> bool {
        match folder {
            None => self.root,
            Some(path) => self.root || self.claimed.contains(path.top_level()),
        }
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
