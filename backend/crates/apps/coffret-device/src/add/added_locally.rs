use std::collections::{BTreeMap, BTreeSet};

use coffret_model::EntryPath;
use coffret_usecase::fetch::local_folder_for;
use coffret_usecase::{root_marker, scratch, FolderEntryKind, MappedRoots};
use tracing::debug;

use super::AddedFile;
use crate::error::Result;
use crate::folder_paths::{child_path, inside};
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// The files in one folder of the Library that this device has and the
    /// Library does not; `None` is the Library root.
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
    /// that folder's answer when somebody opens it. It goes through
    /// [`MappedRoots`](coffret_usecase::MappedRoots), the same capability a
    /// sync's walk reads a mapped folder with, so the two cannot come to
    /// disagree about what is in one — nor about what a name that is a folder or
    /// a symbolic link means (spec: EP-8).
    ///
    /// Three names are left out. Coffret's own scratch, because a half-written
    /// file is not a file anybody put there (see
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
    pub async fn added_locally(&self, folder: Option<&EntryPath>) -> Result<Vec<AddedFile>> {
        // Settled before anything is read, because the answer does not depend on
        // what is there (spec: EP-14).
        if folder.is_some_and(root_marker::carries_management_area) {
            return Ok(Vec::new());
        }
        let Some(directory) = local_folder_for(self.index.as_ref(), folder).await? else {
            return Ok(Vec::new());
        };
        let held: BTreeSet<EntryPath> = self
            .index
            .entries_under(folder)
            .await?
            .into_iter()
            .filter(|location| {
                inside(folder, location.path()).is_some_and(|rest| !rest.contains('/'))
            })
            .map(|location| location.path().clone())
            .collect();

        // A folder that is not there is the `None` the capability answers with,
        // so nothing here reads an error kind to find that out.
        let listed = self
            .local_fs
            .list_folder(directory.mapped_root(), directory.relative())
            .await?;
        let Some(children) = listed else {
            return Ok(Vec::new());
        };

        // Keyed by Entry Path, so the answer comes back in EP-3 order — the byte
        // order of the canonical paths, which is the order the listing beside it
        // is in. A directory read is in whatever order the filesystem felt like.
        let mut rows: BTreeMap<EntryPath, AddedFile> = BTreeMap::new();
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
            if held.contains(&path) {
                continue;
            }
            // A folder is what somebody opens rather than a file to add, and a
            // symbolic link is not something this device may offer the Library
            // at all (spec: EP-8).
            let FolderEntryKind::File { size, mtime, .. } = child.kind else {
                continue;
            };
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
            "read a mapped folder for files the Library does not hold",
        );
        Ok(rows.into_values().collect())
    }
}
