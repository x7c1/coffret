use std::collections::VecDeque;

use crate::folder::Folder;
use crate::reported::Reported;

use super::{Activity, FillStatus};

/// How many stopped runs a later one took the record from are kept, newest
/// last.
///
/// A bound because nothing else ends one of these but somebody taking its
/// folder up, and the ordinary way to make them is not a decision at all: a
/// person clicking from folder to folder while Storage is down stops one fill
/// per click, and every one of them would otherwise ride every answer the
/// activity route gives for the life of the process. Past this many it is the
/// oldest that goes, because it is the one the person has moved furthest from —
/// and what forgetting it costs is a line, not a file: the folder's rows still
/// say `remote`, and opening a file in it brings it over as it always did.
///
/// A count rather than an age. The server keeps no clock of what a browser has
/// read, and one tab's last poll says nothing about another's, so "older than
/// what was last seen" would be a rule about a tab this process cannot see.
///
/// Eight, because that is more lines than the status bar can set out as
/// separate offers and still be read, and fewer than a tab could come to by
/// clicking for a minute. The freeze keeps its own list unbounded, and that is
/// not an oversight: a freeze is a book somebody dropped on purpose into a
/// folder made for it, not a click, and its line is the only thing naming a
/// folder the Library has never heard of.
const DISPLACED_KEPT: usize = 8;

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
    /// The folder a fetch put the fill on next.
    ///
    /// One slot, because a fetch is somebody moving: the folder they are in now
    /// is the one worth having, and the one they were in a moment ago is not
    /// worth queueing behind it. It is also the only thing that supersedes the
    /// run in progress, which is [`superseded`](Self::superseded).
    next: Option<Folder>,
    /// The folders somebody asked for by name, oldest first.
    ///
    /// A queue and not a slot, and not [`next`](Self::next)'s latest-wins
    /// either — the freeze's queue for the same reason. A press of a button is a
    /// decision about that folder rather than a person moving, so a second press
    /// waits behind the first instead of taking its place: two presses bring both
    /// folders over, in the order they were pressed. Displacing the first would
    /// leave the folder it was half way through with nothing on the screen about
    /// it — no line, no button, and no record that it was ever asked for.
    queued: VecDeque<Folder>,
    /// The folders an ending worker threw away, and that nobody has taken up
    /// since.
    ///
    /// Outside [`activity`](Self::activity) because it outlives the fill it was
    /// queued behind: what is dropped is dropped by one run ending and taken up
    /// by a later arming, and a value that went away with the fill on record
    /// would be an offer that vanished the moment somebody pressed the button
    /// beside it.
    ///
    /// A list rather than a slot, although [`next`](Self::next) is one: a second
    /// panic before the first drop was taken up adds to this rather than
    /// replacing it, and a folder nobody has asked for again is not one to
    /// forget about twice.
    dropped: Vec<Folder>,
    /// The runs that stopped and that a later run took the record from, oldest
    /// first, and at most [`DISPLACED_KEPT`] of them: past that, the oldest is
    /// forgotten.
    ///
    /// Outside [`activity`](Self::activity) for the reason
    /// [`dropped`](Self::dropped) is outside it: it outlives the run on record.
    /// A folder Storage stopped half way through is a folder somebody asked for
    /// and did not get whatever the next one does, and
    /// [`activity`](Self::activity) is overwritten the moment the next folder is
    /// taken up — see [`displace`](Self::displace).
    displaced: Vec<Activity>,
    /// How many runs this flow has taken up since the process started.
    ///
    /// What a screen tells one run's account of itself from the next's. It is
    /// here rather than in an [`Activity`] because a run is a folder taken off
    /// the queue, and this is the only thing that sees them all; every activity
    /// published is stamped with it on the way through.
    ///
    /// It is not an identity the Library knows or one anything outside this
    /// process could mean anything by: a server restarted starts again at one.
    runs: u64,
    /// The latest fill, running or finished — what the activity route answers
    /// with.
    pub(super) activity: Option<Activity>,
}

impl Progress {
    /// Follows a fetch into `folder`, and says whether a worker has to be
    /// started for it.
    ///
    /// Latest wins. A fetch that landed in another folder is a person who moved
    /// on, so the fill follows them rather than finishing what they have left;
    /// the folder it was on is not taken up again on its own. Asking for what is
    /// already being brought over is the one thing that changes nothing — except
    /// that it drops a folder a fetch queued behind it, for the same reason:
    /// whoever is asking is here rather than there.
    ///
    /// What it does not displace is [`queued`](Self::queued). Those folders were
    /// asked for by name, and a person walking into a third folder has said
    /// nothing about them; see [`queue`](Self::queue).
    pub(super) fn arm(&mut self, folder: Folder) -> bool {
        // Whoever is asking for it has taken it up, so it is no longer a folder
        // the screen is offering to take up, nor one still waiting its turn, nor
        // a stopped run it is still holding the offer out for — this fetch takes
        // it up now, and running it twice over would be one listing and one run
        // to find every file of it already here. Before the early return below,
        // so that coming back to the folder already being filled settles all of
        // them too.
        self.dropped.retain(|waiting| waiting != &folder);
        self.queued.retain(|asked| asked != &folder);
        self.displaced.retain(|run| run.folder != folder);
        if self.current.as_ref() == Some(&folder) && self.is_filling() {
            self.next = None;
            return false;
        }
        let start = self.announce_if_idle(&folder);
        self.next = Some(folder);
        self.working = true;
        start
    }

    /// Takes `folder` up because somebody asked for it by name, and says whether
    /// a worker has to be started for it.
    ///
    /// The other half of [`arm`](Self::arm), and the half that queues. What
    /// reaches this is a button — the folders a worker that died threw away,
    /// each offered beside the line that named them — and a button pressed on
    /// purpose is not a person moving: pressing a second one must bring that
    /// folder over too rather than erasing the first, which is the whole of why
    /// this is not latest-wins.
    ///
    /// Asking for what is already being brought over, or for what is already
    /// waiting, changes nothing: it is the same folder, and a second run over it
    /// would walk it to find every file already here.
    pub(super) fn queue(&mut self, folder: Folder) -> bool {
        // Whoever pressed the button has taken the offer up, so it is no longer
        // one the screen is making — by either of the two ways it makes one.
        self.dropped.retain(|waiting| waiting != &folder);
        self.displaced.retain(|run| run.folder != folder);
        if self.is_pending(&folder) {
            return false;
        }
        let start = self.announce_if_idle(&folder);
        self.queued.push_back(folder);
        self.working = true;
        start
    }

    /// Whether nothing is running, announcing `folder` as the next run where
    /// nothing is.
    ///
    /// Nothing is running, so nothing else is writing the activity: the fill is
    /// announced as armed rather than leaving the last one's outcome standing
    /// until the worker gets to it. That matters for exactly one caller — the
    /// retry after a fill Storage stopped, which would otherwise be answered
    /// with the failure it is retrying.
    ///
    /// Numbered as the run it is about to become: nothing is running, so the
    /// next `take_next` takes this very folder and counts it.
    fn announce_if_idle(&mut self, folder: &Folder) -> bool {
        let start = !self.working;
        if start {
            self.displace(folder);
            self.activity = Some(self.announce(self.runs + 1, folder.clone()));
        }
        start
    }

    /// The next folder to fill, or nothing — in which case the worker is done
    /// and stops.
    ///
    /// What a fetch armed comes first: it is the folder somebody is looking at
    /// now, and the queue behind it is folders they asked for and are waiting on
    /// rather than reading.
    pub(super) fn take_next(&mut self) -> Option<Folder> {
        match self.next.take().or_else(|| self.queued.pop_front()) {
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
    ///
    /// The folders waiting behind it cannot be left armed — there is no worker
    /// to take them — but they are not forgotten either. They go onto
    /// [`dropped`](Self::dropped), which is what the browser is told about them:
    /// the line and the retry both name the folder that died, and somebody
    /// pressing that one would never learn that the folder they clicked into
    /// afterwards, or the one they asked for by name, was thrown away with it.
    ///
    /// The ones asked for by name come first, in the order they were asked for;
    /// the one a fetch armed is the newest of them and goes last.
    pub(super) fn abandon(&mut self) -> bool {
        if !self.working {
            return false;
        }
        self.working = false;
        self.current = None;
        let waiting: Vec<Folder> = self.queued.drain(..).chain(self.next.take()).collect();
        for folder in waiting {
            if !self.dropped.contains(&folder) {
                self.dropped.push(folder);
            }
        }
        if self.is_filling() {
            if let Some(activity) = self.activity.as_mut() {
                activity.status = FillStatus::Stopped;
                activity.stopped = Some(Reported::unfinished());
            }
        }
        true
    }

    /// Keeps the run on record where it stopped, now that `taken` is taking the
    /// record from it.
    ///
    /// A run that stopped is the one a person still has something to do about:
    /// the folder was asked for and is not here, the line saying so is what the
    /// second attempt hangs off, and the Entries it declined are marked on the
    /// rows out of that same run. Left to be overwritten by the next folder
    /// taken off the queue, all of it would go without being read — and the next
    /// folder is taken up by nothing more unusual than somebody clicking into
    /// another folder while Storage is down.
    ///
    /// Kept here and not on [`dropped`](Self::dropped), although that list is
    /// read back the same way: what is dropped was thrown away before it was
    /// brought over, and a run that walked half the folder is not that. One word
    /// for both would leave a person unable to tell a folder nothing ever
    /// started on from a folder that is half here.
    ///
    /// A run that finished is not kept, and neither is one a fetch superseded:
    /// the first has nothing owing, and the second is a person who walked away
    /// from that folder, which clicking back into arms afresh. Neither is a run
    /// that stopped on the very folder now being taken up — that is the second
    /// attempt at it, and the run it makes says where it is.
    fn displace(&mut self, taken: &Folder) {
        let Some(activity) = self.activity.as_ref() else {
            return;
        };
        if activity.status != FillStatus::Stopped || &activity.folder == taken {
            return;
        }
        // One entry per folder falls out of this rather than being held here: a
        // run is on record because somebody took its folder up, and taking a
        // folder up is exactly what takes it off this list.
        self.displaced.push(activity.clone());
        if self.displaced.len() > DISPLACED_KEPT {
            self.displaced.remove(0);
        }
    }

    /// A fill of `folder` announced as run `run`.
    fn announce(&self, run: u64, folder: Folder) -> Activity {
        Activity {
            run,
            ..Activity::starting(folder)
        }
    }

    /// The folders somebody asked for by name that are still waiting their
    /// turn, oldest first — not counting the one the activity on record is
    /// already about.
    ///
    /// Reported for the reason the freeze's queue is: between the press and the
    /// run, nothing else on the screen is about that folder. The line names the
    /// folder being brought over, and the button that named this one is gone the
    /// moment the queue takes it — so without this a person who pressed one
    /// while a fill was running would watch their press leave no trace at all,
    /// which is what queueing instead of displacing was for.
    ///
    /// What a fetch armed is not among these. [`next`](Self::next) is the fill
    /// moving to where somebody is now rather than a queue anybody made, and the
    /// line follows it of its own accord within a tick.
    ///
    /// Arming is synchronous and taking the folder off the queue is the worker's
    /// first act, so between the two there is a window where the fill on record
    /// names a folder that is still on this queue. Reported as it stands there,
    /// the line would read "bringing over this folder, with this folder after
    /// it". What the activity names in that window is the front of the queue,
    /// which is what nothing being current and nothing being next distinguishes.
    pub(super) fn waiting(&self) -> Vec<Folder> {
        let announced = usize::from(self.current.is_none() && self.next.is_none());
        self.queued.iter().skip(announced).cloned().collect()
    }

    /// The folders an ending worker threw away that nobody has taken up since.
    pub(super) fn dropped(&self) -> &[Folder] {
        &self.dropped
    }

    /// The runs that stopped and that a later one took the record from.
    pub(super) fn displaced(&self) -> &[Activity] {
        &self.displaced
    }

    /// The run the activity on record is, for stamping a published one with.
    pub(super) fn run(&self) -> u64 {
        self.runs
    }

    /// Whether a fetch has landed in another folder, which is what makes the
    /// fill in progress worth abandoning.
    ///
    /// [`queued`](Self::queued) is deliberately not part of this: a folder
    /// somebody asked for by name waits its turn, and reading it as grounds for
    /// abandoning the run would be the second button erasing what the first one
    /// started.
    pub(super) fn superseded(&self) -> bool {
        self.next.is_some()
    }

    /// Whether nothing is being filled and nothing is armed.
    pub(super) fn settled(&self) -> bool {
        !self.working
    }

    /// Whether this folder is the one being filled or one already armed.
    ///
    /// The run has to be under way for the current folder to count: a fill that
    /// stopped is not a fill that is happening, and the retry names the folder
    /// that failed — dropping it as "already being filled" would leave the
    /// browser pressing a button that does nothing.
    fn is_pending(&self, folder: &Folder) -> bool {
        (self.current.as_ref() == Some(folder) && self.is_filling())
            || self.next.as_ref() == Some(folder)
            || self.queued.contains(folder)
    }

    fn is_filling(&self) -> bool {
        self.activity
            .as_ref()
            .is_some_and(|activity| activity.status == FillStatus::Filling)
    }
}

#[cfg(test)]
mod tests {
    use super::{Progress, DISPLACED_KEPT};
    use crate::fill::FillStatus;
    use crate::folder::Folder;

    use crate::entry_paths::entry_path;

    fn folder(path: &str) -> Folder {
        Folder::named(Some(entry_path(path)))
    }

    /// What the worker does: takes a folder, finishes it, and takes the next.
    fn finishes(progress: &mut Progress, status: FillStatus) {
        if let Some(activity) = progress.activity.as_mut() {
            activity.status = status;
        }
    }

    // What a person clicking into a second folder while Storage is down would
    // otherwise cost them. `run::fill` returns normally when Storage stops it,
    // so the worker goes round its loop and takes the next folder — and the run
    // that stopped is the only thing that says the folder is half here, which
    // Entries it declined, and that there is a second attempt to make.
    #[test]
    fn a_folder_storage_stopped_is_kept_once_the_next_one_starts() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        finishes(&mut progress, FillStatus::Stopped);
        progress.queue(folder("books"));

        assert_eq!(progress.take_next(), Some(folder("books")));
        assert_eq!(
            progress
                .displaced()
                .iter()
                .map(|run| (run.run, run.folder.clone(), run.status))
                .collect::<Vec<_>>(),
            [(1, folder("albums"), FillStatus::Stopped)],
            "the folder that stopped is still named, and as the run it was",
        );
    }

    // The same loss by the other road: the worker found nothing armed and
    // stopped, and what takes the record from the stopped run is the next fetch.
    #[test]
    fn a_folder_storage_stopped_is_kept_when_a_later_fetch_arms_another() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        finishes(&mut progress, FillStatus::Stopped);
        assert_eq!(progress.take_next(), None, "the worker leaves");

        assert!(progress.arm(folder("books")), "and another starts");
        assert_eq!(
            progress
                .displaced()
                .iter()
                .map(|run| run.folder.clone())
                .collect::<Vec<_>>(),
            [folder("albums")],
        );
    }

    // And the second attempt at the very folder that stopped is not that: the
    // run it makes is the one to watch, and the failure it is retrying is off
    // the screen from the moment it is armed.
    #[test]
    fn bringing_the_stopped_folder_over_again_leaves_nothing_beside_it() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        finishes(&mut progress, FillStatus::Stopped);
        progress.take_next();

        assert!(progress.queue(folder("albums")));
        assert!(progress.displaced().is_empty());
        progress.take_next();
        assert!(progress.displaced().is_empty());
    }

    // A run that finished has nothing owing, and one a fetch superseded is a
    // person who walked away from that folder — which clicking back into arms
    // afresh. Neither is a folder somebody is still owed.
    #[test]
    fn a_run_that_finished_or_was_superseded_is_not_kept() {
        for status in [FillStatus::Done, FillStatus::Superseded] {
            let mut progress = Progress::default();
            progress.arm(folder("albums"));
            progress.take_next();
            finishes(&mut progress, status);
            progress.arm(folder("books"));
            progress.take_next();

            assert!(progress.displaced().is_empty(), "{status:?} is not kept");
        }
    }

    // The bound `DISPLACED_KEPT` sets, and which end of the list it drops.
    #[test]
    fn only_the_newest_stopped_runs_are_kept() {
        let mut progress = Progress::default();
        let clicked: Vec<String> = (0..=DISPLACED_KEPT + 1)
            .map(|n| format!("albums/{n:02}"))
            .collect();
        for path in &clicked {
            progress.arm(folder(path));
            progress.take_next();
            finishes(&mut progress, FillStatus::Stopped);
            progress.take_next();
        }

        let kept: Vec<Folder> = progress
            .displaced()
            .iter()
            .map(|run| run.folder.clone())
            .collect();
        // The last one clicked is the run on record rather than a displaced one.
        let expected: Vec<Folder> = clicked[clicked.len() - 1 - DISPLACED_KEPT..clicked.len() - 1]
            .iter()
            .map(|path| folder(path))
            .collect();
        assert_eq!(kept, expected, "the newest {DISPLACED_KEPT}, oldest first");
    }

    // Taking one of them up again is what ends its notice, exactly as it is for
    // a folder the queue lost.
    #[test]
    fn asking_for_a_kept_folder_again_takes_it_off_the_list() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        finishes(&mut progress, FillStatus::Stopped);
        progress.arm(folder("books"));
        progress.take_next();

        progress.queue(folder("albums"));
        assert!(progress.displaced().is_empty());
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

    // The rule that separates a button from a person moving: two presses bring
    // both folders over, in the order they were pressed. Latest wins here would
    // be the second press taking the first folder's line, its button and every
    // trace of it off the screen, with half of it brought over.
    #[test]
    fn two_folders_asked_for_by_name_are_both_filled_in_the_order_they_were_asked() {
        let mut progress = Progress::default();
        assert!(progress.queue(folder("books")));
        assert_eq!(progress.take_next(), Some(folder("books")));

        assert!(!progress.queue(folder("letters")));
        assert!(
            !progress.superseded(),
            "the folder being filled is not abandoned for one waiting its turn",
        );
        assert_eq!(progress.take_next(), Some(folder("letters")));
        assert_eq!(progress.take_next(), None);
        assert!(progress.settled());
    }

    // What a browser is told about the queue at all: the folders waiting, in
    // the order they will be brought over. A person who pressed a button while
    // a fill was running is owed the news that theirs is coming — the press
    // takes the button away, and the line names the folder being brought over —
    // and without it the queue is the thing nothing on the screen mentions.
    #[test]
    fn the_folders_asked_for_by_name_are_named_in_the_order_they_will_be_filled() {
        let mut progress = Progress::default();
        progress.queue(folder("books"));
        assert!(
            progress.waiting().is_empty(),
            "the folder the announced fill is about is not also waiting behind itself",
        );
        progress.take_next();
        progress.queue(folder("letters"));
        progress.queue(folder("albums"));

        assert_eq!(progress.waiting(), [folder("letters"), folder("albums")]);

        progress.take_next();
        assert_eq!(progress.waiting(), [folder("albums")]);
    }

    // And what a fetch armed is not one of them: that is the fill moving to
    // where somebody is now, which the line follows by itself — naming it as
    // waiting would be the bar promising a second folder where there is one.
    #[test]
    fn the_folder_a_fetch_armed_is_not_named_as_waiting() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        assert!(progress.waiting().is_empty());

        progress.take_next();
        progress.arm(folder("books"));
        assert!(progress.waiting().is_empty());
    }

    // Asking again for what is already running, or for what is already waiting,
    // says nothing new: it is the same folder, and a second run over it would
    // walk it to find every file already here.
    #[test]
    fn a_folder_already_being_filled_or_already_waiting_is_not_queued_twice() {
        let mut progress = Progress::default();
        progress.queue(folder("books"));
        progress.take_next();

        assert!(!progress.queue(folder("books")));
        assert_eq!(
            progress.take_next(),
            None,
            "nothing was queued behind the run it is already doing",
        );

        progress.queue(folder("letters"));
        progress.take_next();
        progress.queue(folder("albums"));
        progress.queue(folder("albums"));
        assert_eq!(progress.take_next(), Some(folder("albums")));
        assert_eq!(progress.take_next(), None);
    }

    // The two kinds of arming meet at the queue rather than through it. A fetch
    // takes the person where they are now, and the folders they asked for by
    // name are still theirs afterwards — dropping them would be the navigation
    // silently undoing a decision.
    #[test]
    fn following_a_fetch_leaves_the_folders_asked_for_by_name_alone() {
        let mut progress = Progress::default();
        progress.queue(folder("books"));
        progress.take_next();
        progress.queue(folder("letters"));

        progress.arm(folder("albums"));
        assert!(
            progress.superseded(),
            "the fetch is followed, and the run it landed on is abandoned",
        );
        assert_eq!(progress.take_next(), Some(folder("albums")));
        assert_eq!(
            progress.take_next(),
            Some(folder("letters")),
            "the folder asked for by name is still waiting behind it",
        );
    }

    // And a fetch that lands in a folder already waiting takes it up there and
    // then, rather than leaving it queued to be walked a second time.
    #[test]
    fn a_fetch_into_a_folder_already_waiting_takes_it_up_once() {
        let mut progress = Progress::default();
        progress.queue(folder("books"));
        progress.take_next();
        progress.queue(folder("letters"));

        progress.arm(folder("letters"));
        assert_eq!(progress.take_next(), Some(folder("letters")));
        assert_eq!(progress.take_next(), None);
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

    // And the folder that was queued behind it is named rather than thrown away
    // in silence. The line and the retry both belong to the folder that died, so
    // somebody pressing that one would take it up and never learn that the
    // folder they clicked into a moment before was dropped with it.
    #[test]
    fn a_folder_queued_behind_a_worker_that_left_is_named_rather_than_forgotten() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        progress.arm(folder("books"));

        progress.abandon();

        assert_eq!(progress.dropped(), [folder("books")]);
        assert_eq!(
            progress
                .activity
                .as_ref()
                .expect("the fill that died is on record")
                .folder,
            folder("albums"),
            "the fill on record is still the one that died, which is what the line names",
        );
    }

    // Both kinds of waiting are named, because both are folders nothing else on
    // the screen mentions: the ones asked for by name in the order they were
    // asked for, and the one a fetch armed after them last.
    #[test]
    fn every_folder_waiting_behind_a_worker_that_left_is_named() {
        let mut progress = Progress::default();
        progress.queue(folder("books"));
        progress.take_next();
        progress.queue(folder("letters"));
        progress.arm(folder("albums"));

        progress.abandon();

        assert_eq!(progress.dropped(), [folder("letters"), folder("albums")]);
    }

    // And pressing one of those buttons settles that offer, exactly as following
    // a fetch into the folder does.
    #[test]
    fn asking_for_a_dropped_folder_by_name_takes_it_off_the_list() {
        let mut progress = Progress::default();
        progress.queue(folder("books"));
        progress.take_next();
        progress.queue(folder("letters"));
        progress.abandon();
        assert_eq!(progress.dropped(), [folder("letters")]);

        assert!(progress.queue(folder("letters")));
        assert!(progress.dropped().is_empty());
    }

    // What settles the offer is somebody taking that folder up, and nothing
    // else: the run on record ending is not it — the folder was never run at
    // all — so the offer outlives the fill it was queued behind.
    #[test]
    fn taking_a_dropped_folder_up_takes_it_off_the_list() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        progress.arm(folder("books"));
        progress.abandon();

        progress.arm(folder("albums"));
        assert_eq!(
            progress.dropped(),
            [folder("books")],
            "retrying the one that died says nothing about the one that was dropped",
        );

        progress.arm(folder("books"));
        assert!(progress.dropped().is_empty());
    }

    // A second worker leaving before anything was taken up adds to the list
    // rather than replacing it, and a folder already on it is not named twice.
    #[test]
    fn folders_dropped_twice_over_are_each_named_once() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        progress.take_next();
        progress.arm(folder("books"));
        progress.abandon();

        progress.arm(folder("albums"));
        progress.take_next();
        progress.arm(folder("letters"));
        progress.abandon();

        assert_eq!(progress.dropped(), [folder("books"), folder("letters")]);
    }

    // What tells one run's account of itself from the next's. A screen puts a
    // line away by the run it belonged to, so two runs sharing a number would be
    // the second one's line never appearing.
    #[test]
    fn each_folder_taken_up_is_a_run_of_its_own() {
        let mut progress = Progress::default();
        progress.arm(folder("albums"));
        assert_eq!(
            progress
                .activity
                .as_ref()
                .expect("an armed fill is announced")
                .run,
            1,
            "the announced fill is numbered as the run it is about to become",
        );

        progress.take_next();
        assert_eq!(progress.run(), 1);
        progress.arm(folder("books"));
        progress.take_next();
        assert_eq!(progress.run(), 2);
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
