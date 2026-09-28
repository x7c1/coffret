use std::collections::{BTreeMap, BTreeSet};

use coffret_model::EntryPath;
use coffret_usecase::fetch::{local_folder_for, FetchError};
use coffret_usecase::{root_marker, scratch, FolderEntryKind, MappedRoots};
use tracing::debug;

use super::{AddedFile, LocalAdditions};
use crate::error::{Error, Result};
use crate::folder_paths::{child_path, inside};
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// The files in one folder of the Library that this device has and the
    /// Library does not, and the folders standing there that the Library does
    /// not have; `None` is the Library root.
    ///
    /// The mapped folder is read as it stands rather than the catalog being
    /// asked, and it has to be: a file somebody has just put there has no row
    /// anywhere yet — no Entry, because nothing has committed one, and no local
    /// row either, because only a run that materialized a file writes one
    /// (spec: EP-10). The catalog cannot answer a question about a file it has
    /// never heard of, so this looks.
    ///
    /// It is a directory read of one folder and nothing deeper, which is the
    /// same shape as the listing it sits beside: what is under a child folder is
    /// that folder's answer when somebody opens it. A child folder no Entry
    /// stands under is only named (see [`LocalAdditions`]). It goes through
    /// [`MappedRoots`](coffret_usecase::MappedRoots), the same capability a
    /// sync's walk reads a mapped folder with, so the two cannot come to
    /// disagree about what is in one — nor about what a name that is a folder or
    /// a symbolic link means (spec: EP-8).
    ///
    /// Three names are left out, whether a file or a folder stands at them.
    /// Coffret's own scratch, because a half-written file is not a file anybody
    /// put there (see
    /// [`scratch`](coffret_usecase::scratch)) — the whole point of the prefix is
    /// that nothing reads one as user data. The device's own management area,
    /// reserved by name at any depth under a mapped root (spec: EP-14), because
    /// nothing at that name is a file to back up — an ordinary file of the
    /// person's own standing there included — and nothing under a folder of it
    /// is either. Saying so here is what keeps this answer and
    /// [`added_at`](Self::added_at)'s from disagreeing about what a local file of
    /// this device's is. And a name no `str` can be made of, because it spells no
    /// Entry Path (spec: EP-1): a sync reports it rather than backing it up, and
    /// reporting it twice in two vocabularies would put a row on a screen that
    /// nothing can be done with.
    ///
    /// A folder no mapping of this device reaches has no files of its own here,
    /// and neither has one whose mapped folder does not exist — a device that has
    /// mapped a root it has not created yet. Both are an empty answer rather than
    /// a refusal (spec: EP-9): the listing says separately that the folder is not
    /// on this device, which is the sentence a person acts on. A folder whose own
    /// Entry Path carries the reserved name is an empty answer too, on EP-14's
    /// grounds rather than EP-9's.
    ///
    /// A name that only *folds* to the reserved one is the exception to all of
    /// that: it is refused rather than left out, here and for a component of
    /// the folder's own path alike (spec: EP-14). An empty listing is what a
    /// person reads as "there is nothing of mine here", and that is the one
    /// thing nobody can say about a folder a case-folding volume will not tell
    /// apart from coffret's own. The refusal is
    /// [`FoldedReservedComponent`](FetchError::FoldedReservedComponent) and
    /// names that folder, which on the second of the two is a folder standing
    /// *in* the one that was asked about rather than a component of its path.
    ///
    /// # Errors
    ///
    /// [`Error::LocalFilesNotRead`](crate::Error::LocalFilesNotRead) carrying
    /// that verdict, which is the one refusal the translation of a folder
    /// answers with about the folder. The verdict about a component no local name
    /// can be made of belongs to the rule that places a *file* (spec: EP-2,
    /// EP-4), which a folder is deliberately not put through, and a folder no
    /// mapping reaches is the empty answer above rather than a refusal.
    ///
    /// Not [`Error::Fetch`](crate::Error::Fetch), although the vocabulary
    /// inside it is the fetch's: the files this answers about are already on
    /// the disk, and the person on the other end of it opened a folder rather
    /// than asking for a transfer.
    ///
    /// Two more, under their own names rather than inside that one.
    /// [`Error::Index`](crate::Error::Index) where the catalog could not be
    /// read, whichever of the two questions this asks it failed on: the mappings
    /// the translation reads, and which Entries stand under the folder, so
    /// that a file the Library already holds is not reported as one it does
    /// not. Either way nothing was decided about the folder, and a caller is
    /// owed the one shape every entry point reports the catalog in. And
    /// [`Error::Local`](crate::Error::Local) where
    /// the mapped folder is there and the directory read was refused; a folder
    /// that is simply not there is the empty answer above rather than this.
    pub async fn added_locally(&self, folder: Option<&EntryPath>) -> Result<LocalAdditions> {
        // Decided before anything is read, because the answer does not depend on
        // what is there (spec: EP-14).
        if let Some(folder) = folder {
            if let Some(component) = root_marker::component_folding_to_management_area(folder) {
                // Built rather than converted, here and below and at the
                // mappings: `?` on this vocabulary means `Error::Fetch`, and
                // nothing is fetched to read a folder that is already there.
                return Err(Error::LocalFilesNotRead {
                    cause: Box::new(FetchError::FoldedReservedComponent {
                        path: folder.clone(),
                        component: component.to_owned(),
                    }),
                });
            }
        }
        if folder.is_some_and(root_marker::carries_management_area) {
            return Ok(LocalAdditions::default());
        }
        let translated = local_folder_for(self.index.as_ref(), folder)
            .await
            .map_err(Error::local_files_not_read)?;
        let Some(directory) = translated else {
            return Ok(LocalAdditions::default());
        };
        // What the Library holds one level down: the Entries standing in the
        // folder itself, and the child folders the Entries deeper down imply
        // (spec: EP-2). A name in either set is the Library's to list, not this.
        let mut held: BTreeSet<EntryPath> = BTreeSet::new();
        let mut held_folders: BTreeSet<String> = BTreeSet::new();
        for location in self.index.entries_under(folder).await? {
            let Some(rest) = inside(folder, location.path()) else {
                continue;
            };
            match rest.split_once('/') {
                Some((child, _)) => {
                    held_folders.insert(child.to_owned());
                }
                None => {
                    held.insert(location.path().clone());
                }
            }
        }

        // A folder that is not there is the `None` the capability answers with,
        // so nothing here reads an error kind to find that out.
        let listed = self
            .local_fs
            .list_folder(directory.mapped_root(), directory.relative())
            .await?;
        let Some(children) = listed else {
            return Ok(LocalAdditions::default());
        };

        // Keyed by Entry Path, so the answer comes back in EP-3 order — the byte
        // order of the canonical paths, which is the order the listing beside it
        // is in. A directory read is in whatever order the filesystem felt like.
        let mut rows: BTreeMap<EntryPath, AddedFile> = BTreeMap::new();
        let mut folders: BTreeMap<EntryPath, String> = BTreeMap::new();
        for child in children {
            let Some(name) = child.name.to_str().map(str::to_owned) else {
                debug!(
                    operation = "added_locally",
                    "a name in a mapped folder is not UTF-8, and spells no Entry Path",
                );
                continue;
            };
            // Decided from the name alone, the way the scan decides the same
            // question as it walks: the reservation holds at any depth and
            // whatever stands at the name, so an ordinary file called
            // `.coffret` inside a mapped folder is stepped over as surely as
            // the folder holding this device's own marker (spec: EP-11, EP-14).
            if scratch::is_scratch(&name) || root_marker::is_management_area(&name) {
                continue;
            }
            // A name a directory listing returns holds no separator, is never
            // empty, and is never `.` or `..`, so this is the same near-nothing
            // the UTF-8 check above is — and it is passed over the same way: a
            // file this device cannot give a Library position to is not an
            // addition to report (spec: EP-2).
            let Ok(path) = child_path(folder, &name) else {
                debug!(
                    operation = "added_locally",
                    "a name in a mapped folder is no Entry Path component",
                );
                continue;
            };
            // A name that folds to the reserved one without being it, which the
            // skip above deliberately does not cover. The listing is refused
            // whole rather than handed back with the name quietly missing: a
            // case-folding volume gives this listing no way to say whether what
            // stands there is the device's own folder or the person's, and a
            // row left out is a row nobody knows to ask about (spec: EP-14).
            if root_marker::folds_to_management_area(&name) {
                return Err(Error::LocalFilesNotRead {
                    cause: Box::new(FetchError::FoldedReservedComponent {
                        path,
                        component: name,
                    }),
                });
            }
            // A folder is what somebody opens rather than a file to add, so it
            // is answered for by name, and only where the Library has none
            // there: one it has is a row of the listing already. A symbolic link
            // is not something this device may offer the Library at all
            // (spec: EP-8), whatever it points at. The Library's folder is
            // looked for under the name as an Entry Path spells it rather than
            // as the filesystem kept it, the way a file is looked for by its
            // path: a folder written decomposed on disk is the same folder as
            // the composed one the catalog holds (spec: EP-1).
            let (size, mtime) = match child.kind {
                FolderEntryKind::File { size, mtime, .. } => (size, mtime),
                FolderEntryKind::Folder => {
                    if !held_folders.contains(path.name()) {
                        folders.insert(path, name);
                    }
                    continue;
                }
                FolderEntryKind::Other => continue,
            };
            if held.contains(&path) {
                continue;
            }
            rows.insert(
                path.clone(),
                AddedFile {
                    name,
                    path,
                    size,
                    mtime,
                },
            );
        }

        debug!(
            operation = "added_locally",
            library = %self.library_id,
            added = rows.len(),
            folders = folders.len(),
            "read a mapped folder for what the Library does not hold",
        );
        Ok(LocalAdditions {
            files: rows.into_values().collect(),
            folders: folders.into_values().collect(),
        })
    }
}
