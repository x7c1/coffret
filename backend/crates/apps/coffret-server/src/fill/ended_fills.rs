use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::folder::Folder;

use super::{FillStatus, Fills};

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
/// Read off [`Fills::reported`] rather than told by the run that ends, so a
/// run is caught here only while it is still on record — on record itself, or
/// among the stopped runs a later one took the record from. The file route
/// notes what is on record before every arming it makes, which is the moment a
/// finished run is replaced; the one replacement it does not see is a folder
/// queued by `POST /api/fill` taking the record from a run that finished,
/// after which the first page answered from disk in that folder arms one more
/// run.
///
/// Process state, as [`Fills`] is: forgotten when the server stops, so the
/// first page opened after a restart brings the folder over once more.
#[derive(Debug, Default)]
pub(crate) struct EndedFills {
    folders: Mutex<Vec<Folder>>,
}

impl EndedFills {
    /// No run ended on any folder.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Takes in every run `fills` reports as ended: the one on record where it
    /// is done or stopped, and each stopped run a later one took the record
    /// from.
    pub(crate) fn note(&self, fills: &Fills) {
        let Some(latest) = fills.reported() else {
            return;
        };
        let on_record = match latest.on_record.status {
            FillStatus::Done | FillStatus::Stopped(_) => Some(latest.on_record.folder),
            FillStatus::Filling | FillStatus::Superseded => None,
        };
        let mut folders = self.folders();
        for folder in on_record
            .into_iter()
            .chain(latest.displaced.into_iter().map(|kept| kept.run.folder))
        {
            if !folders.contains(&folder) {
                folders.push(folder);
            }
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

#[cfg(test)]
mod tests {
    use super::EndedFills;
    use crate::entry_paths::entry_path;
    use crate::fill::Fills;
    use crate::folder::Folder;

    fn albums() -> Folder {
        Folder::named(Some(entry_path("albums")))
    }

    fn photos() -> Folder {
        Folder::named(Some(entry_path("photos")))
    }

    // A run still on the folder has not ended, and neither has one a fetch
    // into another folder took the worker from: both leave the folder for the
    // next page opened in it to arm.
    #[test]
    fn a_run_filling_or_superseded_is_not_ended() {
        let fills = Fills::new();
        let ended = EndedFills::new();
        assert!(fills.arm(albums(), None, Vec::new()));
        assert_eq!(fills.take_next(), Some(albums()));

        ended.note(&fills);
        assert!(!ended.has_ended(&albums()), "still filling");

        assert!(!fills.arm(photos(), None, Vec::new()));
        assert_eq!(fills.take_next(), Some(photos()));
        ended.note(&fills);
        assert!(!ended.has_ended(&albums()), "superseded, not ended");
        assert!(!ended.has_ended(&photos()), "and the next is still filling");
    }

    // A run the worker abandoned is reported stopped, which is one way a run
    // ends; and once noted, the folder stays noted after the record moves on.
    #[test]
    fn a_stopped_run_is_ended_and_stays_so() {
        let fills = Fills::new();
        let ended = EndedFills::new();
        assert!(fills.arm(albums(), None, Vec::new()));
        assert_eq!(fills.take_next(), Some(albums()));
        fills.abandon();

        ended.note(&fills);
        assert!(ended.has_ended(&albums()));

        assert!(fills.arm(photos(), None, Vec::new()));
        assert_eq!(fills.take_next(), Some(photos()));
        ended.note(&fills);
        assert!(ended.has_ended(&albums()));
        assert!(!ended.has_ended(&photos()));
    }
}
