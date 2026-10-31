use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use coffret_device::{EntryPath, ReservationId};

use crate::api_error::ApiError;
use crate::folder::Folder;

use super::{Add, AddId, Written};

/// Every add this server is carrying in, from its first file to its run's
/// end (spec: LA-34).
///
/// Behind a mutex rather than a watch channel: nothing waits on it, and every
/// writer holds it for one change. The runs that carry adds read it to learn
/// which of theirs a person stopped; the stop route writes that.
#[derive(Debug, Default)]
pub struct Adds {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// How many adds this process has named for a request that named none.
    drawn: u64,
    adds: BTreeMap<AddId, Add>,
}

impl Adds {
    /// None, which is what every process starts with.
    pub fn new() -> Self {
        Self::default()
    }

    /// Begins an add onto `folder` under the name its request gave it, or one
    /// drawn here where it gave none.
    ///
    /// A name another add under way already goes by is refused: a stop asked
    /// by it would not say which of the two it means.
    pub(crate) fn begin(
        &self,
        asked: Option<&str>,
        folder: Folder,
        freeze: bool,
    ) -> Result<AddId, ApiError> {
        let mut inner = self.lock();
        let id = match asked {
            Some(asked) => AddId::named(asked).ok_or_else(ApiError::bad_add)?,
            None => {
                inner.drawn += 1;
                AddId::drawn(inner.drawn)
            }
        };
        if inner.adds.contains_key(&id) {
            return Err(ApiError::add_under_way());
        }
        inner.adds.insert(
            id.clone(),
            Add {
                id: id.clone(),
                folder,
                freeze,
                receiving: true,
                stopping: false,
                files: Vec::new(),
                made: Vec::new(),
                reservation: None,
                reserved: BTreeSet::new(),
                unkept: Vec::new(),
            },
        );
        Ok(id)
    }

    /// Notes that the add is about to write over a file at `path` this device
    /// could not keep aside, and says whether it may: not once a person asked
    /// for the add to be stopped, since nothing could undo that write
    /// (spec: LA-36).
    ///
    /// One decision under the lock a stop is taken under, so a stop and the
    /// write are never both let through: a stop asked before this keeps the
    /// file from being written, and one asked after it is refused
    /// ([`ask_stop`](Self::ask_stop)).
    pub(crate) fn writing_unkept(&self, id: &AddId, path: &EntryPath) -> bool {
        let mut inner = self.lock();
        let Some(add) = inner.adds.get_mut(id) else {
            return true;
        };
        if add.stopping {
            return false;
        }
        add.unkept.push(path.clone());
        true
    }

    /// Takes back one [`writing_unkept`](Self::writing_unkept) whose write did
    /// not happen after all: the file it would have replaced is still there.
    pub(crate) fn not_written_unkept(&self, id: &AddId, path: &EntryPath) {
        if let Some(add) = self.lock().adds.get_mut(id) {
            if let Some(at) = add.unkept.iter().position(|unkept| unkept == path) {
                add.unkept.remove(at);
            }
        }
    }

    /// Notes one file the add wrote, and the folders it made for it.
    pub(crate) fn wrote(&self, id: &AddId, written: Written, made: Vec<PathBuf>) {
        if let Some(add) = self.lock().adds.get_mut(id) {
            add.files.push(written);
            add.made.extend(made);
        }
    }

    /// Notes that the add has finished receiving and its run is armed, with
    /// the reservation its drop holds for a Pack, where it asked for one.
    pub(crate) fn armed(
        &self,
        id: &AddId,
        reservation: Option<(ReservationId, bool)>,
        reserved: BTreeSet<EntryPath>,
    ) {
        if let Some(add) = self.lock().adds.get_mut(id) {
            add.receiving = false;
            add.reservation = reservation;
            add.reserved = reserved;
        }
    }

    /// Ends the add: its run ended, or nothing was armed for it.
    pub(crate) fn end(&self, id: &AddId) {
        self.lock().adds.remove(id);
    }

    /// Ends every add of `ids`, which a run that ended carried.
    pub(crate) fn end_all(&self, ids: &BTreeSet<AddId>) {
        let mut inner = self.lock();
        for id in ids {
            inner.adds.remove(id);
        }
    }

    /// Marks the add stopped by a person, and answers with what it is now —
    /// or `None` where no add of that name is under way.
    ///
    /// Refused, marking nothing, for an add that wrote over a file this device
    /// could not keep aside: undoing it could not put that file back
    /// (spec: LA-36).
    pub(crate) fn ask_stop(&self, id: &AddId) -> Result<Option<Add>, ApiError> {
        let mut inner = self.lock();
        let Some(add) = inner.adds.get_mut(id) else {
            return Ok(None);
        };
        if let Some(unkept) = add.unkept.first() {
            return Err(ApiError::replaced_for_good(&add.folder, unkept));
        }
        add.stopping = true;
        Ok(Some(add.clone()))
    }

    /// Marks every add of `ids` stopped, which a stop of the run carrying them
    /// asks — or none of them, where one wrote over a file this device could
    /// not keep aside ([`ask_stop`](Self::ask_stop)).
    pub(crate) fn ask_stop_all(&self, ids: &BTreeSet<AddId>) -> Result<(), ApiError> {
        let mut inner = self.lock();
        let unkept = ids
            .iter()
            .filter_map(|id| inner.adds.get(id))
            .find_map(|add| add.unkept.first().map(|unkept| (&add.folder, unkept)));
        if let Some((folder, unkept)) = unkept {
            return Err(ApiError::replaced_for_good(folder, unkept));
        }
        for id in ids {
            if let Some(add) = inner.adds.get_mut(id) {
                add.stopping = true;
            }
        }
        Ok(())
    }

    /// Whether a person asked for the add to be stopped.
    pub(crate) fn stopping(&self, id: &AddId) -> bool {
        self.lock().adds.get(id).is_some_and(|add| add.stopping)
    }

    /// Of `ids`, the ones a person asked to be stopped.
    pub(crate) fn stopping_of(&self, ids: &BTreeSet<AddId>) -> BTreeSet<AddId> {
        let inner = self.lock();
        ids.iter()
            .filter(|id| inner.adds.get(*id).is_some_and(|add| add.stopping))
            .cloned()
            .collect()
    }

    /// The add as it stands, for its undo: it stays listed until the undo has
    /// been logged and [`end`](Self::end) takes it out, so a page reading the
    /// work answer finds it in one or the other.
    pub(crate) fn get(&self, id: &AddId) -> Option<Add> {
        self.lock().adds.get(id).cloned()
    }

    /// Every add under way, for the work answer.
    pub(crate) fn listed(&self) -> Vec<Add> {
        self.lock().adds.values().cloned().collect()
    }

    /// The registry, whatever a writer that panicked left it as: every write
    /// is one insert, change or removal, and refusing to answer for the rest
    /// of the process would be worse than any of them half done.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::Adds;
    use crate::folder::Folder;

    // LA-35: a stop names the add, so two adds under way may not share a name.
    #[test]
    fn a_name_is_one_add_at_a_time() {
        let adds = Adds::new();
        let Ok(first) = adds.begin(Some("drop-1"), Folder::named(None), false) else {
            panic!("a fresh name is taken");
        };
        assert!(adds
            .begin(Some("drop-1"), Folder::named(None), false)
            .is_err());
        adds.end(&first);
        assert!(adds
            .begin(Some("drop-1"), Folder::named(None), false)
            .is_ok());
    }

    #[test]
    fn a_stop_marks_the_add_and_nothing_else() {
        let adds = Adds::new();
        let (Ok(one), Ok(two)) = (
            adds.begin(None, Folder::named(None), false),
            adds.begin(None, Folder::named(None), true),
        ) else {
            panic!("a name is drawn for an add whose request named none");
        };
        assert_ne!(one, two);
        assert!(matches!(adds.ask_stop(&one), Ok(Some(_))));
        assert_eq!(
            adds.stopping_of(&BTreeSet::from([one.clone(), two.clone()])),
            BTreeSet::from([one]),
        );
        assert!(!adds.stopping(&two));
    }

    // LA-36: an add that wrote over a file this device could not keep aside
    // is not stopped — not by its own name, and not through its run — and a
    // stop asked first keeps that file from being written over at all.
    #[test]
    fn an_add_that_replaced_a_file_for_good_is_not_stopped() {
        let adds = Adds::new();
        let albums = coffret_device::EntryPath::parse("albums").expect("an Entry Path");
        let (Ok(one), Ok(two)) = (
            adds.begin(None, Folder::named(Some(albums)), false),
            adds.begin(None, Folder::named(None), false),
        ) else {
            panic!("a name is drawn for an add whose request named none");
        };
        let path = coffret_device::EntryPath::parse("albums/cover.png").expect("an Entry Path");
        assert!(adds.writing_unkept(&one, &path));

        // Said as the person's situation: what they asked, and the file why not.
        let refused = adds.ask_stop(&one).expect_err("the stop is refused");
        assert_eq!(
            refused.message(),
            "could not stop adding to albums — albums/cover.png replaced a file that cannot be put back",
        );
        assert!(adds
            .ask_stop_all(&BTreeSet::from([one.clone(), two.clone()]))
            .is_err());
        assert!(!adds.stopping(&one));
        assert!(!adds.stopping(&two), "a refused stop marks none of them");

        adds.not_written_unkept(&one, &path);
        assert!(matches!(adds.ask_stop(&one), Ok(Some(_))));
        assert!(
            !adds.writing_unkept(&one, &path),
            "a stopped add writes over nothing it could not put back",
        );
    }
}
