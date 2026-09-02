use crate::folder::Folder;
use crate::reported::Reported;

use super::{Activity, FillStatus};

/// Everything the server knows about filling folders, in one value.
///
/// One value rather than three, because the three questions are answered
/// together or not at all: what is being filled, what is to be filled next, and
/// what the browser is told about it. It lives behind a
/// [`watch`](tokio::sync::watch) channel, so every change to it is made under
/// one lock and every reader — the activity route, a case waiting for the work
/// to settle — sees a whole answer rather than half of two.
#[derive(Clone, Debug, Default)]
pub(super) struct Progress {
    /// Whether a worker is running at all.
    ///
    /// Not the same question as [`current`](Self::current): a worker is running
    /// from the moment it is spawned, which is before it has taken its first
    /// folder. Two workers would fetch the same Entries twice over, so what
    /// decides whether to spawn one is this rather than what is being filled.
    working: bool,
    /// The folder the worker is on.
    current: Option<Folder>,
    /// The folder it takes up next.
    next: Option<Folder>,
    /// The latest fill, running or finished — what the activity route answers
    /// with.
    pub(super) activity: Option<Activity>,
}

impl Progress {
    /// Makes `folder` what is filled next, and says whether a worker has to be
    /// started for it.
    ///
    /// Latest wins. A fetch that landed in another folder is a person who moved
    /// on, so the fill follows them rather than finishing what they have left;
    /// the folder it was on is not taken up again on its own. Asking for what is
    /// already being brought over is the one thing that changes nothing — except
    /// that it drops a folder queued behind it, for the same reason: whoever is
    /// asking is here rather than there.
    pub(super) fn arm(&mut self, folder: Folder) -> bool {
        if self.current.as_ref() == Some(&folder) && self.is_filling() {
            self.next = None;
            return false;
        }
        let start = !self.working;
        if start {
            // Nothing is running, so nothing else is writing the activity: the
            // fill is announced as armed rather than leaving the last one's
            // outcome standing until the worker gets to it. That matters for
            // exactly one caller — the retry after a fill Storage stopped,
            // which would otherwise be answered with the failure it is
            // retrying.
            self.activity = Some(Activity::starting(folder.clone()));
        }
        self.next = Some(folder);
        self.working = true;
        start
    }

    /// The next folder to fill, or nothing — in which case the worker is done
    /// and stops.
    pub(super) fn take_next(&mut self) -> Option<Folder> {
        match self.next.take() {
            Some(folder) => {
                self.current = Some(folder.clone());
                self.activity = Some(Activity::starting(folder.clone()));
                Some(folder)
            }
            None => {
                self.current = None;
                self.working = false;
                None
            }
        }
    }

    /// Puts back what a worker that ended without taking its leave left set,
    /// and says whether there was anything to put back.
    ///
    /// The way a worker ends is [`take_next`](Self::take_next) finding nothing
    /// armed, which clears all of this itself. The other way it can end is a
    /// panic in the job, and that ends everything rather than one fill: the flag
    /// that says a worker is running is what decides whether to start one, so a
    /// flag nobody clears is a fill route that silently does nothing for the
    /// rest of the process — while the activity goes on saying `filling`, which
    /// is a browser polling a count that will never move and a case waiting on
    /// [`settled`](Self::settled) that will never return.
    ///
    /// So it is left where a fill Storage stopped is left: nothing running, an
    /// activity that says so, and a retry from that state that works, because
    /// the next arming starts a worker again.
    pub(super) fn abandon(&mut self) -> bool {
        if !self.working {
            return false;
        }
        self.working = false;
        self.current = None;
        self.next = None;
        if self.is_filling() {
            if let Some(activity) = self.activity.as_mut() {
                activity.status = FillStatus::Stopped;
                activity.stopped = Some(Reported::unfinished());
            }
        }
        true
    }

    /// Whether another folder is waiting, which is what makes the fill in
    /// progress worth abandoning.
    pub(super) fn superseded(&self) -> bool {
        self.next.is_some()
    }

    /// Whether nothing is being filled and nothing is armed.
    pub(super) fn settled(&self) -> bool {
        !self.working
    }

    fn is_filling(&self) -> bool {
        self.activity
            .as_ref()
            .is_some_and(|activity| activity.status == FillStatus::Filling)
    }
}

#[cfg(test)]
mod tests {
    use super::Progress;
    use crate::fill::FillStatus;
    use crate::folder::Folder;
    use coffret_device::EntryPath;

    fn folder(path: &str) -> Folder {
        Folder::named(Some(EntryPath::nfc(path)))
    }

    /// What the worker does: takes a folder, finishes it, and takes the next.
    fn finishes(progress: &mut Progress, status: FillStatus) {
        if let Some(activity) = progress.activity.as_mut() {
            activity.status = status;
        }
    }

    #[test]
    fn the_first_arming_starts_a_worker_and_the_second_does_not() {
        let mut progress = Progress::default();
        assert!(progress.arm(folder("albums")));
        assert!(!progress.arm(folder("books")));
    }

    // Latest wins: the fill follows whoever is clicking rather than finishing
    // the folder they have left.
    #[test]
    fn a_second_folder_supersedes_the_one_being_filled() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        assert_eq!(progress.take_next(), Some(folder("albums")));
        assert!(!progress.superseded());

        progress.arm(folder("books"));
        assert!(progress.superseded());
        assert_eq!(progress.take_next(), Some(folder("books")));
    }

    // Clicking a second file of the folder already being brought over changes
    // nothing, and takes back a folder queued behind it.
    #[test]
    fn arming_the_folder_being_filled_changes_nothing() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();

        assert!(!progress.arm(folder("albums")));
        assert!(!progress.superseded());

        progress.arm(folder("books"));
        progress.arm(folder("albums"));
        assert!(
            !progress.superseded(),
            "coming back to what is running takes the other folder off the list",
        );
    }

    // A fill that stopped is not a fill that is running, whichever folder is
    // asked for: the retry after a Storage error names the folder that failed,
    // and dropping it as "already being filled" would leave the browser
    // pressing a button that does nothing.
    #[test]
    fn the_folder_a_stopped_fill_was_on_can_be_armed_again() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        finishes(&mut progress, FillStatus::Stopped);

        assert!(!progress.arm(folder("albums")));
        assert!(
            progress.superseded(),
            "the retry is armed rather than dropped"
        );
    }

    // A worker that ended any other way than by finding nothing armed panicked,
    // and everything it left set has to be put back: a flag nobody clears is a
    // worker no arming ever starts again, and an activity left filling is one a
    // browser follows for the rest of the process's life.
    #[test]
    fn a_worker_that_ends_without_taking_its_leave_leaves_nothing_running() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();

        assert!(progress.abandon());
        assert!(progress.settled());
        let activity = progress
            .activity
            .as_ref()
            .expect("a fill that was armed is on record");
        assert_eq!(activity.status, FillStatus::Stopped);
        assert!(
            activity.stopped.is_some(),
            "the browser is told what became of it, and is offered the retry",
        );
        assert!(
            progress.arm(folder("albums")),
            "the folder can be taken up again, and starts a worker",
        );
    }

    // The ordinary ending puts itself back, so there is nothing here to undo —
    // least of all the outcome the fill came to.
    #[test]
    fn a_worker_that_took_its_leave_leaves_what_it_finished_alone() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        finishes(&mut progress, FillStatus::Done);
        progress.take_next();

        assert!(!progress.abandon());
        assert_eq!(
            progress
                .activity
                .as_ref()
                .expect("the fill that ran is on record")
                .status,
            FillStatus::Done,
        );
    }

    #[test]
    fn a_worker_that_finds_nothing_armed_stops() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        assert!(!progress.settled());

        assert_eq!(progress.take_next(), None);
        assert!(progress.settled());
        assert!(
            progress.arm(folder("albums")),
            "the next arming starts a worker again",
        );
    }
}
