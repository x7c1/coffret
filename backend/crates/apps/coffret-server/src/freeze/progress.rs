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
    /// The books an ending worker threw away, and that nobody has asked for
    /// since.
    ///
    /// Outside [`activity`](Self::activity) because it outlives the run they
    /// were queued behind, exactly as the fill's does: what is dropped is
    /// dropped by one run ending and taken up by a later arming, and an offer
    /// that went away with the run on record would vanish the moment somebody
    /// pressed the button beside it.
    dropped: Vec<Folder>,
    /// The runs that stopped and that a later run took the record from, oldest
    /// first.
    ///
    /// Outside [`activity`](Self::activity) for the reason
    /// [`dropped`](Self::dropped) is outside it: it outlives the run on record.
    /// A book Storage stopped is a folder of pages sitting outside the Library
    /// whatever the next book does, and [`activity`](Self::activity) is
    /// overwritten the moment the queue is taken up — see
    /// [`displace`](Self::displace).
    displaced: Vec<FreezeActivity>,
    /// How many runs this flow has taken up since the process started.
    ///
    /// What a screen tells one book's account of itself from the next's. It is
    /// here rather than in a [`FreezeActivity`] because a run is a folder taken
    /// off the queue, and this is the only thing that sees them all; every
    /// activity published is stamped with it on the way through.
    runs: u64,
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
        // Whoever is asking for it has taken it up, so it is no longer a book
        // the screen is offering to take up, nor a stopped run it is still
        // holding the offer out for: this arming is that second attempt, and
        // what it makes is the run on record.
        self.dropped.retain(|book| book != &folder);
        self.displaced.retain(|run| run.folder != folder);
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
            // Numbered as the run it is about to become: nothing is running, so
            // the next `take_next` takes this very folder and counts it.
            self.displace(&folder);
            self.activity = Some(self.announce(self.runs + 1, folder.clone()));
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
                self.runs += 1;
                self.current = Some(folder.clone());
                self.displace(&folder);
                self.activity = Some(self.announce(self.runs, folder.clone()));
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
    /// the next arming starts a worker again. The books that were waiting come
    /// off the queue with it — there is no worker left to take them — and go
    /// onto [`dropped`](Self::dropped) rather than being forgotten, because a
    /// person who queued a second book behind the first is owed the news that it
    /// never started.
    pub(super) fn abandon(&mut self) -> bool {
        if !self.working {
            return false;
        }
        self.working = false;
        self.current = None;
        for book in self.waiting.drain(..) {
            if !self.dropped.contains(&book) {
                self.dropped.push(book);
            }
        }
        if self.is_freezing() {
            if let Some(activity) = self.activity.as_mut() {
                activity.status = FreezeStatus::Stopped;
                activity.stopped = Some(Reported::unfinished());
                // A step is where a run that is *running* has got to, and this
                // one is over — which is what `FreezeActivity::step` says it
                // means and what the browser is told it means. A run that ends
                // the ordinary way clears it by publishing its own finished
                // value; this one never reached that, so the last phase it
                // reported would stand here as a phase nothing is in any more.
                activity.step = None;
            }
        }
        true
    }

    /// Keeps the run on record where it stopped, now that `taken` is taking the
    /// record from it.
    ///
    /// A run that stopped is the one a person still has something to do about:
    /// its pages are sitting in the folder and out of the Library, the folder
    /// itself was never anything but the browser's, and the line naming it is
    /// the only thing on any screen that says so. Left to be overwritten by the
    /// next run, it would be in none of the three lists a browser reads back —
    /// so a reload would draw no row for that folder, offer no way to walk into
    /// it, and make no second attempt at it, with the pages still on the disk.
    /// A second book queued behind the first is enough to reach that, and the
    /// browser queues one whenever somebody drops two books in a session.
    ///
    /// Kept here and not on [`dropped`](Self::dropped), although that list is
    /// read back the same way: what is dropped was thrown away before it was
    /// packed, and a run that ran and was stopped is not that. One word for both
    /// would leave a person unable to tell a book nothing ever started on from a
    /// book that went half way up.
    ///
    /// A run that finished is not kept: its batch committed, the Library names
    /// the folder, and the next run starting is the end of what it had to say.
    /// Neither is one that stopped on the very folder now being taken up — that
    /// is the second attempt at it, and the run it makes says where it is.
    fn displace(&mut self, taken: &Folder) {
        let Some(activity) = self.activity.as_ref() else {
            return;
        };
        if activity.status != FreezeStatus::Stopped || &activity.folder == taken {
            return;
        }
        // One entry per book falls out of this rather than being held here: a
        // run is on record because somebody took its folder up, and taking a
        // folder up is exactly what takes that book off this list.
        self.displaced.push(activity.clone());
    }

    /// A freeze of `folder` announced as run `run`.
    fn announce(&self, run: u64, folder: Folder) -> FreezeActivity {
        FreezeActivity {
            run,
            ..FreezeActivity::starting(folder)
        }
    }

    /// The run the activity on record is, for stamping a published one with.
    pub(super) fn run(&self) -> u64 {
        self.runs
    }

    /// The books waiting their turn, oldest first — not counting the one the
    /// activity on record is already about.
    ///
    /// Arming is synchronous and taking the book off the queue is the worker's
    /// first act, so between the two there is a window where the freeze on
    /// record names a book that is still on this queue. Reported as it stands
    /// there, the line would read "packing this book, with this book after it".
    /// What the activity names in that window is the front of the queue, which
    /// is what [`current`](Self::current) being empty distinguishes.
    pub(super) fn waiting(&self) -> Vec<Folder> {
        let announced = usize::from(self.current.is_none());
        self.waiting.iter().skip(announced).cloned().collect()
    }

    /// The books an ending worker threw away that nobody has asked for since.
    pub(super) fn dropped(&self) -> &[Folder] {
        &self.dropped
    }

    /// The runs that stopped and that a later one took the record from.
    pub(super) fn displaced(&self) -> &[FreezeActivity] {
        &self.displaced
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
    use coffret_device::{Phase, Step};

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

    /// What the flow does while the run is under way: says where it has got to.
    fn reports(progress: &mut Progress, step: Step) {
        if let Some(activity) = progress.activity.as_mut() {
            activity.step = Some(step);
        }
    }

    // The failure two books in one session is enough to reach. `run::freeze`
    // returns normally when Storage stops it, so the worker goes round its loop
    // and takes the next book off the queue — and the book that stopped is in a
    // folder the browser made, which the Library has never heard of. Overwritten,
    // it would be in none of the lists a page reads back: its pages would sit on
    // the disk with no row in the tree, no way to walk in, and no second attempt.
    #[test]
    fn a_book_storage_stopped_is_kept_once_the_next_one_starts() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();
        finishes(&mut progress, FreezeStatus::Stopped);
        progress.arm(folder("books/vol-2"));

        assert_eq!(progress.take_next(), Some(folder("books/vol-2")));
        assert_eq!(
            progress
                .displaced()
                .iter()
                .map(|run| (run.run, run.folder.clone(), run.status))
                .collect::<Vec<_>>(),
            [(1, folder("books/vol-1"), FreezeStatus::Stopped)],
            "the book that stopped is still named, and as the run it was",
        );
        assert_eq!(
            progress.activity.as_ref().map(|activity| activity.status),
            Some(FreezeStatus::Freezing),
            "while the run on record is the one being packed now",
        );
    }

    // The same loss by the other road: the worker found nothing waiting and
    // stopped, and what takes the record from the stopped run is the next drop
    // rather than the next turn of the loop.
    #[test]
    fn a_book_storage_stopped_is_kept_when_a_later_drop_arms_another() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();
        finishes(&mut progress, FreezeStatus::Stopped);
        assert_eq!(progress.take_next(), None, "the worker leaves");

        assert!(progress.arm(folder("books/vol-2")), "and another starts");
        assert_eq!(
            progress
                .displaced()
                .iter()
                .map(|run| run.folder.clone())
                .collect::<Vec<_>>(),
            [folder("books/vol-1")],
        );
    }

    // And the second attempt at the very book that stopped is not that. It is
    // the same book, so the run it makes is the one to watch and a line about
    // the failure beside it would be the bar saying the book is both packing
    // and not packed.
    #[test]
    fn packing_the_stopped_book_again_leaves_nothing_beside_the_run_it_makes() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();
        finishes(&mut progress, FreezeStatus::Stopped);
        progress.take_next();

        assert!(progress.arm(folder("books/vol-1")));
        assert!(progress.displaced().is_empty());
        progress.take_next();
        assert!(progress.displaced().is_empty());
        assert_eq!(
            progress.activity.as_ref().map(|activity| activity.status),
            Some(FreezeStatus::Freezing),
        );
    }

    // A book that committed has nothing owing: the Library names its folder, and
    // the next book starting is the end of what it had to say.
    #[test]
    fn a_book_that_committed_is_not_kept_when_the_next_one_starts() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();
        finishes(&mut progress, FreezeStatus::Done);
        progress.arm(folder("books/vol-2"));
        progress.take_next();

        assert!(progress.displaced().is_empty());
    }

    // One Storage outage stops every book queued behind the first, and each of
    // them is a folder of pages outside the Library. Reported one at a time, all
    // but the last would go unsaid.
    #[test]
    fn every_book_a_stopped_run_left_behind_is_named() {
        let mut progress = Progress::default();
        for book in ["books/vol-1", "books/vol-2", "books/vol-3"] {
            progress.arm(folder(book));
        }
        for _ in 0..3 {
            progress.take_next();
            finishes(&mut progress, FreezeStatus::Stopped);
        }

        assert_eq!(
            progress
                .displaced()
                .iter()
                .map(|run| run.folder.clone())
                .collect::<Vec<_>>(),
            [folder("books/vol-1"), folder("books/vol-2")],
            "the two the record was taken from, with the third still on it",
        );
    }

    // Taking one of them up again is what ends its notice, exactly as it is for
    // a book the queue lost.
    #[test]
    fn asking_for_a_kept_book_again_takes_it_off_the_list() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();
        finishes(&mut progress, FreezeStatus::Stopped);
        progress.arm(folder("books/vol-2"));
        progress.take_next();

        progress.arm(folder("books/vol-1"));
        assert!(progress.displaced().is_empty());
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
        reports(&mut progress, Step::new(Phase::Packing, 2, 5));

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
            activity.step.is_none(),
            "a run that is over is in no phase, whichever way it ended",
        );
        assert!(
            progress.arm(folder("books/vol-1")),
            "the folder can be taken up again, and starts a worker",
        );
    }

    // And the book that was waiting behind it is named rather than thrown away
    // in silence, for the reason the fill's queued folder is: the line and the
    // retry both belong to the run that died, so the second book would be lost
    // with nothing on the screen having mentioned it.
    #[test]
    fn a_book_waiting_behind_a_worker_that_left_is_named_rather_than_forgotten() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        progress.take_next();
        progress.arm(folder("books/vol-2"));

        progress.abandon();

        assert!(
            progress.waiting().is_empty(),
            "there is no worker to take it"
        );
        assert_eq!(progress.dropped(), [folder("books/vol-2")]);

        progress.arm(folder("books/vol-2"));
        assert!(
            progress.dropped().is_empty(),
            "and asking for it again is what settles the offer",
        );
    }

    // What a browser is told about a queue at all: the books waiting, in the
    // order they will be packed in. A person who dropped the second one is owed
    // the news that theirs is queued, which a count of one cannot give them.
    #[test]
    fn the_books_waiting_are_named_in_the_order_they_will_be_packed() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        assert!(
            progress.waiting().is_empty(),
            "the book the announced freeze is about is not also waiting behind itself",
        );
        progress.take_next();
        progress.arm(folder("books/vol-2"));
        progress.arm(folder("books/vol-3"));

        assert_eq!(
            progress.waiting(),
            [folder("books/vol-2"), folder("books/vol-3")],
        );

        progress.take_next();
        assert_eq!(progress.waiting(), [folder("books/vol-3")]);
    }

    // What tells one book's account of itself from the next's, so that a line
    // somebody has read and put away does not take the next book's with it.
    #[test]
    fn each_book_taken_up_is_a_run_of_its_own() {
        let mut progress = Progress::default();
        progress.arm(folder("books/vol-1"));
        assert_eq!(
            progress
                .activity
                .as_ref()
                .expect("an armed freeze is announced")
                .run,
            1,
        );

        progress.take_next();
        assert_eq!(progress.run(), 1);
        progress.arm(folder("books/vol-2"));
        progress.take_next();
        assert_eq!(progress.run(), 2);
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
