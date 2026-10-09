use std::collections::VecDeque;

use coffret_device::Step;
use tokio::sync::watch;

use crate::reported::Reported;

use super::{DeleteRun, DeleteStatus, Target};

/// What the server is deleting from the Library, and what it deleted last.
#[derive(Debug)]
pub struct Deletes {
    /// The one place any of this is written, so that every reader sees a whole
    /// answer and a case can wait on one.
    progress: watch::Sender<Progress>,
}

/// The latest deletion, and how many are waiting behind it, read together.
#[derive(Clone, Debug)]
pub struct DeleteReport {
    /// The run on record: the one running, or the last one to end.
    pub on_record: DeleteRun,
    /// How many deletions are waiting their turn behind it.
    pub waiting: usize,
}

impl Default for Deletes {
    fn default() -> Self {
        Self::new()
    }
}

impl Deletes {
    /// Nothing being deleted.
    pub fn new() -> Self {
        Self {
            progress: watch::channel(Progress::default()).0,
        }
    }

    /// The latest deletion and the queue behind it, under one borrow — and
    /// `None` where no deletion has run.
    ///
    /// A finished deletion is kept rather than cleared, because what a browser
    /// most needs from it is what a finished one says: what left the Library,
    /// and what was refused.
    pub fn reported(&self) -> Option<DeleteReport> {
        let progress = self.progress.borrow();
        Some(DeleteReport {
            on_record: progress.on_record.clone()?,
            waiting: progress.waiting(),
        })
    }

    /// Whether a deletion is running or waiting to run.
    pub fn running(&self) -> bool {
        self.progress.borrow().working
    }

    /// Waits until nothing is being deleted and nothing is waiting.
    ///
    /// What a case drives the work with: arming is synchronous, so a case that
    /// has armed one has already put it on this value by the time it waits.
    pub async fn until_idle(&self) {
        let mut watched = self.progress.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait; a channel that closed anyway leaves nothing to wait
        // for.
        let _ = watched.wait_for(|progress| !progress.working).await;
    }

    /// Asks for `target` to be deleted, and says whether a worker has to be
    /// started for it. See [`Progress::arm`].
    pub(super) fn arm(&self, target: Target) -> bool {
        let mut start = false;
        self.progress
            .send_modify(|progress| start = progress.arm(target));
        start
    }

    /// The next deletion to run, or nothing — in which case the worker is done
    /// and stops.
    pub(super) fn take_next(&self) -> Option<Target> {
        let mut taken = None;
        self.progress
            .send_modify(|progress| taken = progress.take_next());
        taken
    }

    /// Puts back what a worker that ended without taking its leave left set.
    /// See [`Progress::abandon`].
    pub(super) fn abandon(&self) {
        self.progress.send_if_modified(Progress::abandon);
    }

    /// Says what the deletion in progress came to.
    pub(super) fn publish(&self, run: &DeleteRun) {
        self.progress.send_modify(|progress| {
            progress.on_record = Some(DeleteRun {
                run: progress.runs,
                ..run.clone()
            });
        });
    }

    /// Says how far into the running deletion the flow has got.
    pub(super) fn step(&self, step: Step) {
        self.progress.send_modify(|progress| {
            if let Some(run) = progress.on_record.as_mut() {
                run.step = Some(step);
            }
        });
    }
}

/// Everything the server knows about deleting, in one value behind one lock.
#[derive(Clone, Debug, Default)]
struct Progress {
    /// Whether a worker is running at all — from the moment it is spawned,
    /// which is before it has taken its first deletion. Two workers would race
    /// two batches over one catalog into the commit, so what decides whether to
    /// spawn one is this.
    working: bool,
    /// The deletion the worker is on.
    current: Option<Target>,
    /// The deletions it takes up after it, oldest first.
    ///
    /// A queue and not "latest wins": every deletion here was confirmed by a
    /// person, and a second one superseding the first would be a confirmation
    /// thrown away.
    waiting: VecDeque<Target>,
    /// How many runs have been taken up since the process started.
    runs: u64,
    /// The latest deletion, running or finished.
    on_record: Option<DeleteRun>,
}

impl Progress {
    /// Asks for `target` to be deleted, and says whether a worker has to be
    /// started for it.
    ///
    /// Asking for exactly what is already running or waiting changes nothing:
    /// a second press of one confirmation is not a second deletion.
    fn arm(&mut self, target: Target) -> bool {
        if self.current.as_ref() == Some(&target) || self.waiting.contains(&target) {
            return false;
        }
        let start = !self.working;
        if start {
            // Nothing is running, so nothing else is writing the run on record:
            // the deletion is announced as armed rather than leaving the last
            // one's outcome standing until the worker gets to it.
            self.on_record = Some(DeleteRun {
                run: self.runs + 1,
                ..DeleteRun::starting(target.clone())
            });
        }
        self.waiting.push_back(target);
        self.working = true;
        start
    }

    /// The next deletion to run, or nothing.
    fn take_next(&mut self) -> Option<Target> {
        match self.waiting.pop_front() {
            Some(target) => {
                self.runs += 1;
                self.current = Some(target.clone());
                self.on_record = Some(DeleteRun {
                    run: self.runs,
                    ..DeleteRun::starting(target.clone())
                });
                Some(target)
            }
            None => {
                self.current = None;
                self.working = false;
                None
            }
        }
    }

    /// Puts back what a worker that panicked left set, and says whether there
    /// was anything to put back.
    ///
    /// The deletions waiting behind it go with it: there is no worker left to
    /// take them, and each committed nothing, so every Entry they named is
    /// still in the Library for the person to delete again.
    fn abandon(&mut self) -> bool {
        if !self.working {
            return false;
        }
        self.working = false;
        self.current = None;
        self.waiting.clear();
        if let Some(run) = self.on_record.as_mut() {
            if run.status == DeleteStatus::Deleting {
                run.status = DeleteStatus::Stopped(Reported::unfinished());
                run.step = None;
            }
        }
        true
    }

    /// How many deletions are waiting, not counting the one the run on record
    /// already announces while the worker has yet to take it up.
    fn waiting(&self) -> usize {
        let announced = usize::from(self.current.is_none());
        self.waiting.len().saturating_sub(announced)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::Deletes;
    use crate::delete::{DeleteStatus, Target};
    use crate::entry_paths::entry_path;
    use crate::reported::Reported;

    fn folder(path: &str) -> Target {
        Target {
            folder: Some(entry_path(path)),
            paths: BTreeSet::new(),
        }
    }

    // A second deletion confirmed while the first runs waits its turn, and a
    // second press of the same one is not queued again.
    #[test]
    fn a_second_deletion_waits_and_the_same_one_is_not_queued_twice() {
        let deletes = Deletes::new();
        assert!(deletes.arm(folder("albums")));
        assert_eq!(deletes.take_next(), Some(folder("albums")));

        assert!(!deletes.arm(folder("books")), "one worker, already running");
        assert!(!deletes.arm(folder("books")));
        assert!(!deletes.arm(folder("albums")), "the running one");
        let report = deletes.reported().expect("a deletion was armed");
        assert_eq!(report.waiting, 1);
        assert_eq!(report.on_record.target, folder("albums"));

        assert_eq!(deletes.take_next(), Some(folder("books")));
        assert_eq!(deletes.take_next(), None);
        assert!(!deletes.running());
    }

    // A worker that died takes its queue with it and says the run stopped,
    // rather than leaving a browser following a deletion nothing is running.
    #[test]
    fn abandoning_a_deletion_says_it_stopped_and_tells_whoever_waits() {
        let deletes = Deletes::new();
        assert!(deletes.arm(folder("albums")));
        deletes.take_next();
        deletes.arm(folder("books"));

        let mut watched = deletes.progress.subscribe();
        drop(watched.borrow_and_update());
        deletes.abandon();

        assert!(watched.has_changed().expect("the sender outlives the case"));
        let report = deletes.reported().expect("a deletion was armed");
        assert_eq!(
            report.on_record.status,
            DeleteStatus::Stopped(Reported::unfinished())
        );
        assert_eq!(report.waiting, 0);
        assert!(!deletes.running());
        assert!(
            deletes.arm(folder("books")),
            "and the next arming starts a worker"
        );
    }
}
