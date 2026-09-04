use std::collections::{BTreeMap, BTreeSet};
use std::io;

use coffret_model::EntryPath;
use coffret_usecase::fetch::local_folder_for;
use coffret_usecase::{mtime_of, scratch};
use tokio::fs;
use tracing::debug;

use super::AddedFile;
use crate::error::{Error, Result};
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
    /// that folder's answer when somebody opens it.
    ///
    /// Two names are left out. Coffret's own scratch, because a half-written file
    /// is not a file anybody put there (see
    /// [`scratch`](coffret_usecase::scratch)) — the whole point of the prefix is
    /// that nothing reads one as user data. And a name no `str` can be made of,
    /// because it spells no Entry Path (spec: EP-1): a sync reports it rather
    /// than backing it up, and reporting it twice in two vocabularies would put
    /// a row on a screen that nothing can be done with.
    ///
    /// A folder no mapping of this device reaches has no files of its own here,
    /// and neither has one whose mapped folder does not exist — a device that has
    /// mapped a root it has not created yet. Both are an empty answer rather than
    /// a refusal (spec: EP-9): the listing says separately that the folder is not
    /// on this device, which is the sentence a person acts on.
    pub async fn added_locally(&self, folder: Option<&EntryPath>) -> Result<Vec<AddedFile>> {
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

        let mut reading = match fs::read_dir(&directory).await {
            Ok(reading) => reading,
            Err(absent) if absent.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(cause) => {
                return Err(Error::local("a mapped folder could not be read", directory)(cause))
            }
        };

        // Keyed by Entry Path, so the answer comes back in EP-3 order — the byte
        // order of the canonical paths, which is the order the listing beside it
        // is in. A directory read is in whatever order the filesystem felt like.
        let mut rows: BTreeMap<EntryPath, AddedFile> = BTreeMap::new();
        loop {
            let child = match reading.next_entry().await {
                Ok(Some(child)) => child,
                Ok(None) => break,
                Err(cause) => {
                    return Err(Error::local("a mapped folder could not be read", directory)(cause))
                }
            };
            let Some(name) = child.file_name().to_str().map(str::to_owned) else {
                debug!(
                    operation = "added_locally",
                    "a name in a mapped folder is not UTF-8, and spells no Entry Path",
                );
                continue;
            };
            if scratch::is_scratch(&name) {
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
            // The metadata is asked for after the name is, because a file that
            // went between the two is one that is no longer there to report.
            let metadata = match child.metadata().await {
                Ok(metadata) => metadata,
                Err(gone) if gone.kind() == io::ErrorKind::NotFound => continue,
                Err(cause) => {
                    return Err(Error::local(
                        "a file in a mapped folder could not be read",
                        child.path(),
                    )(cause))
                }
            };
            if !metadata.is_file() {
                continue;
            }
            rows.insert(
                path.clone(),
                AddedFile {
                    name,
                    path,
                    size: metadata.len(),
                    mtime: mtime_of(&metadata),
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
