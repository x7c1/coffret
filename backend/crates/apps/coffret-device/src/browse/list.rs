use std::collections::BTreeSet;

use coffret_model::{ContainerId, ContainerKind, EntryPath, LibraryId};
use coffret_usecase::device_state::Mapping;
use tracing::{debug, warn};

use super::{ChildFolder, EntryState, FileRow, FolderListing};
use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// What one folder holds, one level down; `None` is the Library root
    /// (spec: LA-44).
    ///
    /// Two questions of the catalog and no more: what the folder holds, and
    /// which parts of the Library this device has a folder for at all. The
    /// first is answered by the Index out of the folder's own rows — its child
    /// folders with what each comes to at every depth, and the Entries
    /// standing directly in it with whether this device has each and the kind
    /// of Container each lives in — never out of the subtree under it, so the
    /// listing costs what it answers with however much lies below
    /// (spec: LA-45). A row's state comes from a present materialization
    /// record and from nothing else, because a mapping asserts nothing about
    /// what is on disk (spec: EP-9, EP-10) — the mappings answer the separate
    /// question of whether anything here could be fetched, which is
    /// [`mapped`](FolderListing::mapped).
    ///
    /// An Entry standing at exactly the folder's own path is not in the listing.
    /// The Library admits one — nothing stops a file and a folder sharing a
    /// path, since neither is a thing the Library has — and it is not a child of
    /// itself; whoever named the folder is who has it.
    ///
    /// A folder the Library does not have is not refused here. It answers with
    /// empty lists, as it must — a folder is an implication of the paths, so
    /// there is nothing to look up and nothing to be missing — and says which of
    /// the two an empty answer is in [`held`](FolderListing::held).
    pub async fn list(&self, folder: Option<&EntryPath>) -> Result<FolderListing> {
        let contents = self.index.folder_contents(folder).await?;
        let mappings = self.index.mappings().await?;
        // The top-level folders this device maps, which the root's listing
        // names apart from its folders where nothing is under one yet.
        let prefixes: BTreeSet<EntryPath> = match folder {
            None => mappings
                .iter()
                .filter_map(|mapping| mapping.prefix.clone())
                .collect(),
            Some(_) => BTreeSet::new(),
        };
        let reach = Reach::of(mappings);

        let folders: Vec<ChildFolder> = contents
            .folders
            .into_iter()
            .map(|child| ChildFolder {
                name: child.path.name().to_owned(),
                mapped: reach.reaches(Some(&child.path)),
                // Every Entry below it is here exactly where the count of
                // those here reaches the count of them all.
                present_throughout: child.files_present == child.files_total,
                files_total: child.files_total,
                files_present: child.files_present,
                size: child.size,
                mtime: child.mtime,
                path: child.path,
            })
            .collect();
        let files: Vec<FileRow> = contents
            .files
            .into_iter()
            .map(|file| FileRow {
                name: file.location.path().name().to_owned(),
                path: file.location.path().clone(),
                size: file.location.entry.extent.size(),
                mtime: file.location.entry.mtime,
                deposited_at: file.location.deposited_at,
                state: match file.present {
                    true => EntryState::Present,
                    false => EntryState::Remote,
                },
                container: kind_of(file.container, file.location.container_id, self.library_id),
            })
            .collect();

        // A mapped top-level folder that holds no Entry is not a folder of the
        // Library (spec: EP-2), and it is still where this device has room for
        // one: the explorer offers it as a place to drop into.
        let named: BTreeSet<&EntryPath> = folders.iter().map(|child| &child.path).collect();
        let empty_mapped_prefixes: Vec<EntryPath> = prefixes
            .into_iter()
            .filter(|prefix| !named.contains(prefix))
            .collect();

        debug!(
            operation = "list",
            library = %self.library_id,
            folders = folders.len(),
            files = files.len(),
            empty_mapped_prefixes = empty_mapped_prefixes.len(),
            "listed one folder of the Library",
        );
        Ok(FolderListing {
            mapped: reach.reaches(folder),
            // A folder is there because something is under it, so the rows are
            // the answer: one child of any kind and the Library has this folder,
            // none and it has never had it. The root is the exception and not a
            // special case of the rule — it is not a path the catalog implies,
            // it is the Library — so it is there whether or not anything is in
            // it, which is the state every Library starts in.
            held: folder.is_none() || !folders.is_empty() || !files.is_empty(),
            path: folder.cloned(),
            folders,
            files,
            empty_mapped_prefixes,
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
    /// is out of reach, since it represents whatever no other mapping does.
    root: bool,
    /// The top-level components a mapping of their own represents.
    represented: BTreeSet<String>,
}

impl Reach {
    /// What this device's mappings come to, as the question a listing asks.
    ///
    /// Owned rather than borrowed from the mappings, because the listing holds
    /// the answer past the call that read them.
    fn of(mappings: Vec<Mapping>) -> Self {
        Self {
            root: mappings.iter().any(|mapping| mapping.prefix.is_none()),
            represented: Mapping::represented_prefixes(&mappings)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }

    /// Whether a mapping reaches one folder, `None` being the Library root.
    ///
    /// The root is reached by the root mapping alone: a top-level mapping
    /// stands for its own subtree and not for what sits beside it, so a device
    /// that maps only `albums` has nowhere to put a file that lives at the top
    /// of the Library. Every other folder is reached by whichever mapping
    /// represents its top-level component, or by the root mapping where none
    /// does.
    fn reaches(&self, folder: Option<&EntryPath>) -> bool {
        match folder {
            None => self.root,
            Some(path) => self.root || self.represented.contains(path.top_level()),
        }
    }
}

/// Which kind of Container one Entry lives in (spec: PK-15).
///
/// The Index answers with the Entry and the kind of its Container together,
/// and an adapter reading them in two reads may meet another process
/// committing between them: a Container that stopped being current after the
/// Entries were read is a Container this listing has an Entry for and no
/// summary of. It is rare, it is nobody's mistake, and the next listing will
/// not have it — but it is not nothing either, because what reads this field
/// decides from it whether an Entry can be replaced one file at a time. So it
/// is reported, and the row falls back to the answer that refuses nothing on
/// its own.
fn kind_of(
    kind: Option<ContainerKind>,
    container_id: ContainerId,
    library: LibraryId,
) -> ContainerKind {
    match kind {
        Some(kind) => kind,
        None => {
            warn!(
                operation = "list",
                library = %library,
                container = %container_id,
                "the catalog holds an Entry in a Container it summarizes no longer",
            );
            ContainerKind::OneFile
        }
    }
}
