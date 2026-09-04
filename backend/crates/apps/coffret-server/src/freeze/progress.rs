use std::collections::VecDeque;

use crate::folder::Folder;
use crate::reported::Reported;

use super::{FreezeActivity, FreezeStatus};

/// Everything the server knows about freezing folders, in one value.
///
/// One value rather than three, because the three questions are answered
/// together or not at all: what is being packed, what is to be packed after it,
/// and what the browser is told about it. It lives behind a
/// [`watch`](tokio::sync::watch) channel, so every change to it is made under
/// one lock and every reader — the activity route, a case waiting for the work
/// to settle — sees a whole answer rather than half of two.
#[derive(Clone, Debug, Default)]
pub(super) struct Progress {
    /// Whether a worker is running at all.
    ///
    /// Not the same question as [`current`](Self::current): a worker is running
    /// from the moment it is spawned, which is before it has taken its first
    /// folder. Two workers would build two batches out of one folder's files and
    /// race each other into the commit, so what decides whether to spawn one is
    /// this.
    working: bool,
    /// The folder the worker is on.
    current: Option<Folder>,
    /// The folders it takes up after it, oldest first.
    ///
    /// A queue and not a single slot, and not the fill's "latest wins" either. A
    /// freeze commits one batch (spec: PK-7), so a book displaced half way
    /// through is one that was never brought in at all: a second book waits
    /// rather than taking the running one's place, and a third waits behind it
    /// rather than pushing the second off.
    waiting: VecDeque<Folder>,
    /// The latest freeze, running or finished — what the activity route answers
    /// with.
    pub(super) activity: Option<FreezeActivity>,
}

impl Progress {
    /// Asks for `folder` to be packed, and says whether a worker has to be
    /// started for it.
    ///
    /// Asking for what is already being packed, or for what is already waiting,
    /// changes nothing: a second drop into the same folder is the same book, and
    /// a second run over it would find every file of it packed already
    /// (spec: PK-2) at the cost of another walk.
    pub(super) fn arm(&mut self, folder: Folder) -> bool {
        if self.is_pending(&folder) {
            return false;
        }
        let start = !self.working;
        if start {
            // Nothing is running, so nothing else is writing the activity: the
            // freeze is announced as armed rather than leaving the last one's
            // outcome standing until the worker gets to it. That matters for
            // exactly one caller — the retry after a freeze Storage stopped,
            // which would otherwise be answered with the failure it is retrying.
            self.activity = Some(FreezeActivity::starting(folder.clone()));
        }
        self.waiting.push_back(folder);
        self.working = true;
        start
    }

    /// The next folder to pack, or nothing — in which case the worker is done
    /// and stops.
    pub(super) fn take_next(&mut self) -> Option<Folder> {
        match self.waiting.pop_front() {
            Some(folder) => {
                self.current = Some(folder.clone());
                self.activity = Some(FreezeActivity::starting(folder.clone()));
                Some(folder)
            }
            None => {
                self.current = None;
                self.working = false;
                None
            }
        }
    }

    /// Puts back what a worker that ended without taking its leave left set, and
    /// says whether there was anything to put back.
    ///
    /// The way a worker ends is [`take_next`](Self::take_next) finding nothing
    /// waiting, which clears all of this itself. The other way it can end is a
    /// panic in the job, and that ends everything rather than one run: the flag
    /// that says a worker is running is what decides whether to start one, so a
    /// flag nobody clears is a drop that silently packs nothing for the rest of
    /// the process — while the activity goes on saying `freezing` to a browser
    /// that polls it and a case waits on a settling that never comes.
    ///
    /// So it is left where a freeze Storage stopped is left: nothing running, an
    /// activity that says so, and a retry from that state that works, because
    /// the next arming starts a worker again. The books that were waiting are
    /// dropped with it — nothing has been said about them and nobody is watching
    /// them, and re-arming the one that failed is what the browser is offered.
    pub(super) fn abandon(&mut self) -> bool {
        if !self.working {
            return false;
        }
        self.working = false;
        self.current = None;
        self.waiting.clear();
        if self.is_freezing() {
            if let Some(activity) = self.activity.as_mut() {
                activity.status = FreezeStatus::Stopped;
                activity.stopped = Some(Reported::unfinished());
            }
        }
        true
    }

    /// Whether nothing is being packed and nothing is waiting.
    pub(super) fn settled(&self) -> bool {
        !self.working
    }

    /// Whether this folder is the one being packed or one already waiting.
    ///
    /// The run has to be under way for the current folder to count: a freeze
    /// that stopped is not a freeze that is happening, and the retry names the
    /// folder that failed — dropping it as "already being packed" would leave
    /// the browser pressing a button that does nothing.
    fn is_pending(&self, folder: &Folder) -> bool {
        (self.current.as_ref() == Some(folder) && self.is_freezing())
            || self.waiting.contains(folder)
    }

    fn is_freezing(&self) -> bool {
        self.activity
            .as_ref()
            .is_some_and(|activity| activity.status == FreezeStatus::Freezing)
    }
}

#[cfg(test)]
mod tests {
    use super::Progress;
    use crate::folder::Folder;
    use crate::freeze::FreezeStatus;

    use crate::entry_paths::entry_path;

    fn folder(path: &str) -> Folder {
        Folder::named(Some(entry_path(path)))
    }

    /// What the worker does: takes a folder and finishes it.
    fn finishes(progress: &mut Progress, status: FreezeStatus) {
        if let Some(activity) = progress.activity.as_mut() {
            activity.status = status;
        }
    }

    #[test]
    fn the_first_arming_starts_a_worker_and_the_second_does_not() {
        let mut progress = Progress::default();
        assert!(progress.arm(folder("books/vol-1")));
        assert!(!progress.arm(folder("books/vol-2")));
    }

    // The rule the whole module turns on, and the one that separates it from the
    // fill: a book that is being packed is finished before the next one starts,
    // because a freeze commits one batch and a book abandoned half way through
    // it is one that was never brought in at all.
    #[test]
    fn a_second_book_waits_rather_than_taking_the_first_ones_place() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        assert_eq!(progress.take_next(), Some(folder("books/vol-1")));

        progress.arm(folder("books/vol-2"));
        assert_eq!(progress.take_next(), Some(folder("books/vol-2")));
        assert_eq!(progress.take_next(), None);
        assert!(progress.settled());
    }

    // A second drop into the folder being packed is the same book, and a second
    // run over it would walk it again to find every file already packed
    // (spec: PK-2).
    #[test]
    fn asking_again_for_the_book_being_packed_queues_nothing() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();

        assert!(!progress.arm(folder("books/vol-1")));
        assert_eq!(
            progress.take_next(),
            None,
            "nothing was queued behind the run it is already doing",
        );
    }

    #[test]
    fn a_folder_already_waiting_is_not_queued_twice() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();

        progress.arm(folder("books/vol-2"));
        progress.arm(folder("books/vol-2"));
        assert_eq!(progress.take_next(), Some(folder("books/vol-2")));
        assert_eq!(progress.take_next(), None);
    }

    // A freeze that stopped is not a freeze that is running: the retry after a
    // Storage error names the folder that failed, and dropping it as "already
    // being packed" would leave the browser pressing a button that does nothing.
    #[test]
    fn the_folder_a_stopped_freeze_was_on_can_be_armed_again() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();
        finishes(&mut progress, FreezeStatus::Stopped);

        assert!(!progress.arm(folder("books/vol-1")), "a worker is still on");
        assert_eq!(
            progress.take_next(),
            Some(folder("books/vol-1")),
            "the retry is armed rather than dropped",
        );
    }

    // A worker that ended any other way than by finding nothing waiting
    // panicked, and everything it left set has to be put back: a flag nobody
    // clears is a worker no drop ever starts again, and an activity left
    // freezing is one a browser follows for the rest of the process's life.
    #[test]
    fn a_worker_that_ends_without_taking_its_leave_leaves_nothing_running() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();

        assert!(progress.abandon());
        assert!(progress.settled());
        let activity = progress
            .activity
            .as_ref()
            .expect("a freeze that was armed is on record");
        assert_eq!(activity.status, FreezeStatus::Stopped);
        assert!(
            activity.stopped.is_some(),
            "the browser is told what became of it, and is offered the retry",
        );
        assert!(
            progress.arm(folder("books/vol-1")),
            "the folder can be taken up again, and starts a worker",
        );
    }

    // The ordinary ending puts itself back, so there is nothing here to undo —
    // least of all the outcome the run came to.
    #[test]
    fn a_worker_that_took_its_leave_leaves_what_it_finished_alone() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();
        finishes(&mut progress, FreezeStatus::Done);
        progress.take_next();

        assert!(!progress.abandon());
        assert_eq!(
            progress
                .activity
                .as_ref()
                .expect("the freeze that ran is on record")
                .status,
            FreezeStatus::Done,
        );
    }
}
