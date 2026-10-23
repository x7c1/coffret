use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::folder::Folder;

use super::{FillRun, FillStatus};

/// The folders a fill run has ended on in this process, whether it brought
/// everything over or Storage stopped it.
///
/// What a page answered from disk asks before it arms a fill (see
/// `fill_unless_whole` in the file route). A folder a run has ended on is one
/// the fill has already had its go at: what is still not here after it is an
/// Entry the fill declined or one Storage would not hand over, and arming
/// again on every page turned would be one run per page meeting the same
/// answer. A page that has to be fetched still arms, and so does the retry.
///
/// A run a fetch into another folder superseded has not ended in this sense:
/// it left the folder half brought over, and coming back to it arms it afresh.
///
/// Told by the run as it ends — [`Fills`](super::Fills) records it in the same
/// stroke that publishes the ending, and in the one that puts back a worker
/// that died — rather than read off what is on record, so no run is missed
/// because a later one took the record from it first.
///
/// Process state, as [`Fills`](super::Fills) is: forgotten when the server
/// stops, so the first page opened after a restart brings the folder over once
/// more.
#[derive(Debug, Default)]
pub(crate) struct EndedFills {
    folders: Mutex<Vec<Folder>>,
}

impl EndedFills {
    /// No run ended on any folder.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Takes in `run` where it has ended: done, or stopped.
    pub(crate) fn note(&self, run: &FillRun) {
        match run.status {
            FillStatus::Done | FillStatus::Stopped(_) => {}
            FillStatus::Filling | FillStatus::Superseded => return,
        }
        let mut folders = self.folders();
        if !folders.contains(&run.folder) {
            folders.push(run.folder.clone());
        }
    }

    /// Whether a run has been noted as ended on `folder`.
    pub(crate) fn has_ended(&self, folder: &Folder) -> bool {
        self.folders().contains(folder)
    }

    /// The list, read through a lock a panicking writer cannot poison: a push
    /// is never left half done.
    fn folders(&self) -> MutexGuard<'_, Vec<Folder>> {
        self.folders.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
