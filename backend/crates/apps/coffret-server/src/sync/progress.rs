use super::{SyncActivity, SyncStatus};
use crate::reported::Reported;

/// Everything the server knows about syncing, in one value.
///
/// One value rather than three, because the three questions are answered
/// together or not at all: whether a worker is running, whether another run is
/// wanted, and what the browser is told about it. It lives behind a
/// [`watch`](tokio::sync::watch) channel, so every change to it is made under one
/// lock and every reader — the activity route, a case waiting for the work to
/// settle — sees a whole answer rather than half of two.
#[derive(Clone, Debug, Default)]
pub(super) struct Progress {
    /// Whether a worker is running at all.
    ///
    /// Not the same question as [`armed`](Self::armed): a worker is running from
    /// the moment it is spawned, which is before it has taken its first run. Two
    /// workers would walk one set of folders twice over and race each other into
    /// the commit, so what decides whether to spawn one is this.
    working: bool,
    /// Whether another run is wanted once the current one is over.
    ///
    /// A flag and not a count, which is the whole of the collapsing rule: two
    /// drops during one sync queue one follow-up, because the next run walks the
    /// mapped folders and one walk finds both files.
    armed: bool,
    /// The latest sync, running or finished — what the activity route answers
    /// with.
    pub(super) activity: Option<SyncActivity>,
}

impl Progress {
    /// Asks for a sync, and says whether a worker has to be started for it.
    ///
    /// Never dropped, unlike a fill's arming: a fill of another folder replaces
    /// the one running because the person has moved on, and there is no such
    /// thing here — a sync asked for during a sync is asking about files the
    /// running one may already have walked past.
    pub(super) fn arm(&mut self) -> bool {
        let start = !self.working;
        if start {
            // Nothing is running, so nothing else is writing the activity: the
            // sync is announced as armed rather than leaving the last one's
            // outcome standing until the worker gets to it. That matters for
            // exactly one caller — the retry after a sync Storage stopped, which
            // would otherwise be answered with the failure it is retrying.
            self.activity = Some(SyncActivity::starting());
        }
        self.armed = true;
        self.working = true;
        start
    }

    /// Whether there is a run to take up, in which case the worker takes it — and
    /// otherwise the worker is done and stops.
    pub(super) fn take_next(&mut self) -> bool {
        if self.armed {
            self.armed = false;
            self.activity = Some(SyncActivity::starting());
            return true;
        }
        self.working = false;
        false
    }

    /// Puts back what a worker that ended without taking its leave left set, and
    /// says whether there was anything to put back.
    ///
    /// The way a worker ends is [`take_next`](Self::take_next) finding nothing
    /// armed, which clears all of this itself. The other way it can end is a
    /// panic in the job, and that ends everything rather than one run: the flag
    /// that says a worker is running is what decides whether to start one, so a
    /// flag nobody clears is a drop that silently syncs nothing for the rest of
    /// the process — while the activity goes on saying `syncing` to a browser
    /// that polls it and a case waits on a settling that never comes.
    ///
    /// So it is left where a sync Storage stopped is left: nothing running, an
    /// activity that says so, and a retry from that state that works, because the
    /// next arming starts a worker again.
    pub(super) fn abandon(&mut self) -> bool {
        if !self.working {
            return false;
        }
        self.working = false;
        self.armed = false;
        if self.is_syncing() {
            if let Some(activity) = self.activity.as_mut() {
                activity.status = SyncStatus::Stopped;
                activity.stopped = Some(Reported::unfinished());
            }
        }
        true
    }

    /// Whether nothing is being synced and nothing is armed.
    pub(super) fn settled(&self) -> bool {
        !self.working
    }

    fn is_syncing(&self) -> bool {
        self.activity
            .as_ref()
            .is_some_and(|activity| activity.status == SyncStatus::Syncing)
    }
}

#[cfg(test)]
mod tests {
    use super::Progress;
    use crate::sync::SyncStatus;

    /// What the worker does: takes a run and finishes it.
    fn finishes(progress: &mut Progress, status: SyncStatus) {
        if let Some(activity) = progress.activity.as_mut() {
            activity.status = status;
        }
    }

    #[test]
    fn the_first_arming_starts_a_worker_and_the_second_does_not() {
        let mut progress = Progress::default();
        assert!(progress.arm());
        assert!(!progress.arm());
    }

    // The rule the whole module turns on: what a drop during a running sync buys
    // is one more walk of the mapped folders, and a second drop buys nothing
    // further, because that walk finds both files.
    #[test]
    fn two_drops_during_one_sync_queue_exactly_one_run() {
        let mut progress = Progress::default();
        progress.arm();
        assert!(progress.take_next(), "the first run is taken up");

        progress.arm();
        progress.arm();
        assert!(progress.take_next(), "the follow-up run is taken up");
        assert!(
            !progress.take_next(),
            "and there is not a second follow-up behind it",
        );
        assert!(progress.settled());
    }

    // A sync that stopped is not a sync that is running: the retry after a
    // Storage error has to arm one, and dropping it as "already syncing" would
    // leave the browser pressing a button that does nothing.
    #[test]
    fn a_stopped_sync_can_be_armed_again() {
        let mut progress = Progress::default();
        progress.arm();
        progress.take_next();
        finishes(&mut progress, SyncStatus::Stopped);

        assert!(!progress.arm(), "a worker is still running the loop");
        assert!(
            progress.take_next(),
            "the retry is armed rather than dropped"
        );
    }

    // A worker that ended any other way than by finding nothing armed panicked,
    // and everything it left set has to be put back: a flag nobody clears is a
    // worker no drop ever starts again, and an activity left syncing is one a
    // browser follows for the rest of the process's life.
    #[test]
    fn a_worker_that_ends_without_taking_its_leave_leaves_nothing_running() {
        let mut progress = Progress::default();
        progress.arm();
        progress.take_next();

        assert!(progress.abandon());
        assert!(progress.settled());
        let activity = progress
            .activity
            .as_ref()
            .expect("a sync that was armed is on record");
        assert_eq!(activity.status, SyncStatus::Stopped);
        assert!(
            activity.stopped.is_some(),
            "the browser is told what became of it, and is offered the retry",
        );
        assert!(
            progress.arm(),
            "a sync can be asked for again, and starts a worker",
        );
    }

    // The ordinary ending puts itself back, so there is nothing here to undo —
    // least of all the outcome the run came to.
    #[test]
    fn a_worker_that_took_its_leave_leaves_what_it_finished_alone() {
        let mut progress = Progress::default();
        progress.arm();
        progress.take_next();
        finishes(&mut progress, SyncStatus::Done);
        progress.take_next();

        assert!(!progress.abandon());
        assert_eq!(
            progress
                .activity
                .as_ref()
                .expect("the sync that ran is on record")
                .status,
            SyncStatus::Done,
        );
    }
}
