use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coffret_model::Redacted;
use tracing::warn;

use crate::device_state::RootMarkerId;
use crate::local_trash::LocalTrash;
use crate::mapped_relative_location::MappedRelativeLocation;

/// The folders a run's discards may have emptied, and the removal of
/// the ones they did (spec: EP-15).
///
/// Only a folder that held a discarded file is ever a candidate, and each
/// folder above it below the mapped root: a folder that was already empty
/// before the run held nothing the run discarded, so it is never asked about,
/// and a folder that still holds anything — an edited copy, a file of the
/// person's, a dotfile, a folder — is not removed by the gateway, which removes
/// a folder only where nothing at all is in it.
#[derive(Debug, Default)]
pub(crate) struct EmptiedFolders {
    /// Keyed by the root and the folder's path below it, so a folder several
    /// discarded files stood in is asked about once.
    candidates: BTreeMap<(PathBuf, PathBuf), Candidate>,
}

#[derive(Debug)]
struct Candidate {
    relative: MappedRelativeLocation,
    expected: Option<RootMarkerId>,
    depth: usize,
}

impl EmptiedFolders {
    /// Records that the file at `relative` below `root` was discarded into the
    /// trash.
    pub(crate) fn discarded(
        &mut self,
        root: &Path,
        expected: Option<RootMarkerId>,
        relative: &MappedRelativeLocation,
    ) {
        let mut above = relative.parent();
        while let Some(folder) = above {
            let key = (root.to_path_buf(), folder.to_path_buf());
            if self.candidates.contains_key(&key) {
                // Every folder above this one was recorded with it.
                break;
            }
            above = folder.parent();
            self.candidates.insert(
                key,
                Candidate {
                    depth: folder.components().count(),
                    relative: folder,
                    expected,
                },
            );
        }
    }

    /// Removes every candidate that holds nothing at all, the deepest first,
    /// so a folder is asked about only after every folder below it was.
    ///
    /// A refusal is logged and goes no further: the folder stays as it was, a
    /// folder above it then holds it and stays too, and nothing else in the run
    /// depends on it. Nothing asks again — the next run discarded nothing out
    /// of it — so the person removes such a folder the way they would any empty
    /// folder of their own.
    pub(crate) async fn remove(self, trash: &dyn LocalTrash) {
        let mut candidates: Vec<_> = self.candidates.into_iter().collect();
        candidates.sort_by_key(|(_, candidate)| Reverse(candidate.depth));
        for ((root, _), candidate) in candidates {
            if let Err(refused) = trash
                .remove_emptied_folder(&root, candidate.expected.as_ref(), &candidate.relative)
                .await
            {
                // What refused and of what kind, and nothing that names the
                // folder, for the reason a refused discard names no file.
                warn!(
                    reason = %refused.redacted(),
                    "a folder the discards emptied could not be removed; it is left in place",
                );
            }
        }
    }
}
