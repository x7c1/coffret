use std::sync::Arc;

use coffret_device::Step;
use tokio::sync::watch;

use crate::activity_log::{ActivityLog, Ended, Queue};
use crate::folder::Folder;
use crate::latest::Latest;

use super::progress::Progress;
use super::{Book, FreezeRun};

/// What the server is packing into the Library, and what it packed last.
#[derive(Debug)]
pub struct Freezes {
    /// The one place any of this is written, so that every reader sees a whole
    /// answer and a case can wait on one.
    progress: watch::Sender<Progress>,
    /// Where every run that ends is logged, shared with the rest of the
    /// server's work (see [`ActivityLog`]).
    log: Arc<ActivityLog>,
}

impl Default for Freezes {
    fn default() -> Self {
        Self::new()
    }
}

impl Freezes {
    /// Nothing being packed.
    pub fn new() -> Self {
        Self {
            progress: watch::channel(Progress::default()).0,
            log: Arc::new(ActivityLog::new()),
        }
    }

    /// Nothing being packed, logging what ends into `log` — the server's one
    /// log of its finished work, which [`new`](Self::new) keeps apart for a
    /// case that drives this flow alone.
    pub fn logging_to(log: Arc<ActivityLog>) -> Self {
        Self { log, ..Self::new() }
    }

    /// The latest freeze, the runs it took the record from, the books waiting
    /// behind it and the ones its queue lost — and `None` where no freeze has
    /// run.
    ///
    /// A finished freeze is kept rather than cleared, because the two things a
    /// browser most needs from this are things a finished freeze says: what the
    /// run left alone, and whether Storage stopped it — the state the retry is
    /// offered from. The three lists are read beside it rather than out of it,
    /// because none of them is any one run's property: the book on record is the
    /// one being packed, the queue is what nothing has been said about yet, what
    /// a worker that died threw away outlives the run it was queued behind, and
    /// a run Storage stopped goes on being a folder of pages outside the Library
    /// after the next book has taken the record from it.
    ///
    /// All four under one borrow, which is the whole reason they live in one
    /// value. They change together: a worker that dies marks its run stopped and
    /// moves the books behind it onto the discarded list in the same stroke, a book
    /// taken off the queue becomes the run on record and puts the stopped one it
    /// replaced onto the displaced list in another, and a drop that finds nothing
    /// running announces the run and queues the book in a third. Read one at a
    /// time, an answer could carry half of any of them — a run still saying
    /// `freezing` beside the books that very ending threw away, or a run still
    /// saying `done` beside a queue already taken up, which is a browser told
    /// there is nothing left to follow at the moment the next book starts.
    pub fn reported(&self) -> Option<Latest<FreezeRun>> {
        let progress = self.progress.borrow();
        let run = progress.on_record.clone()?;
        Some(Latest {
            on_record: run,
            displaced: progress.displaced().to_vec(),
            waiting: progress.waiting(),
            discarded: progress.discarded(),
        })
    }

    /// Whether a freeze is running or waiting to run.
    ///
    /// What the drop route asks before answering, so that a browser is told
    /// there is something to follow.
    pub fn running(&self) -> bool {
        !self.progress.borrow().idle()
    }

    /// Waits until nothing is being packed and nothing is waiting.
    ///
    /// What a case drives the work with. Arming is synchronous, so a case whose
    /// drop has landed has already put the freeze on this value by the time it
    /// awaits here — which is what lets the cases assert on a finished freeze
    /// without sleeping on one.
    pub async fn until_idle(&self) {
        let mut watched = self.progress.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait; a channel that closed anyway leaves nothing to wait
        // for.
        let _ = watched.wait_for(Progress::idle).await;
    }

    /// Asks for `book` to be packed, and says whether a worker has to be
    /// started for it. See [`Progress::arm`].
    pub(super) fn arm(&self, book: Book) -> bool {
        let mut start = false;
        self.progress
            .send_modify(|progress| start = progress.arm(book));
        start
    }

    /// Asks for the book a drop just finished writing to be packed, as a run
    /// of its own. See [`Progress::arm_dropped`].
    pub(super) fn arm_dropped(&self, book: Book) -> bool {
        let mut start = false;
        self.progress
            .send_modify(|progress| start = progress.arm_dropped(book));
        start
    }

    /// What packing `folder` again asks for: the files the runs and books still
    /// kept for it were asked to pack, or every file under it where nothing is
    /// kept (spec: PK-17).
    pub fn again(&self, folder: Folder) -> Book {
        self.progress.borrow().again(folder)
    }

    /// The next book to pack, or nothing — in which case the worker is done
    /// and stops. See [`Progress::take_next`].
    pub(super) fn take_next(&self) -> Option<Book> {
        let mut taken = None;
        self.progress
            .send_modify(|progress| taken = progress.take_next());
        taken
    }

    /// Puts back what a worker that ended without taking its leave left set.
    ///
    /// For a worker that panicked, and for no other: the ordinary ending clears
    /// all of this itself, and by the time it is over another worker may own
    /// the flag. Notified only where there was something to put back, which is
    /// why this goes through [`send_if_modified`](watch::Sender::send_if_modified).
    ///
    /// A run it stops is logged as one that stopped through
    /// [`publish`](Self::publish) is, and every book it throws away is logged
    /// as lost.
    pub(super) fn abandon(&self) {
        self.progress.send_if_modified(|progress| {
            let freezing = progress.is_freezing();
            let lost_before = progress.discarded();
            let modified = progress.abandon();
            if modified && freezing {
                if let Some(run) = &progress.on_record {
                    self.log.record(Ended::Freeze(run.clone()));
                }
            }
            for folder in progress.discarded() {
                if !lost_before.contains(&folder) {
                    self.log.record(Ended::Discarded {
                        queue: Queue::Freeze,
                        folder,
                    });
                }
            }
            modified
        });
    }

    /// Says where the freeze in progress has got to.
    ///
    /// The run number is stamped on here rather than carried by the caller: the
    /// value a run builds is its own account of one book, and which run of the
    /// flow that is is the queue's to say.
    pub(super) fn publish(&self, run: &FreezeRun) {
        self.progress.send_modify(|progress| {
            let published = FreezeRun {
                run: progress.run(),
                ..run.clone()
            };
            // Logged under the same lock that publishes it, so no reader sees
            // the run over before the log has it.
            if !matches!(published.status, super::FreezeStatus::Freezing) {
                self.log.record(Ended::Freeze(published.clone()));
            }
            progress.on_record = Some(published);
        });
    }

    /// Says how far into the running freeze the flow has got.
    ///
    /// Written onto the run on record rather than published as one, because
    /// what reports it is the flow itself while the run's own value is still
    /// being built: the two meet when the run finishes and publishes.
    pub(super) fn step(&self, step: Step) {
        self.progress.send_modify(|progress| {
            if let Some(run) = progress.on_record.as_mut() {
                run.step = Some(step);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::Freezes;
    use crate::folder::Folder;
    use crate::freeze::{Book, FreezeStatus};
    use crate::reported::Reported;

    use crate::entry_paths::entry_path;

    fn book() -> Book {
        Book::whole(Folder::named(Some(entry_path("books/vol-1"))))
    }

    // The half of a worker's leaving that `Progress` cannot state: putting the
    // state back is of no use to anyone unless the change is sent. What waits on
    // it is a case awaiting `until_idle` and, through the work route, a browser
    // polling for the run to end — and `send_if_modified` sends nothing at all
    // where the closure reports nothing changed, so a freeze abandoned without a
    // notification is exactly the wait that never ends.
    #[test]
    fn abandoning_a_freeze_tells_whoever_is_waiting_on_it() {
        let freezes = Freezes::new();
        assert!(freezes.arm(book()));
        assert_eq!(freezes.take_next(), Some(book()));

        let mut watched = freezes.progress.subscribe();
        drop(watched.borrow_and_update());
        freezes.abandon();

        assert!(
            watched.has_changed().expect("the sender outlives the case"),
            "a wait for the freeze to finish is ended by this and by nothing else",
        );
        let latest = freezes
            .reported()
            .expect("a freeze that was armed is on record");
        assert_eq!(
            latest.on_record.status,
            FreezeStatus::Stopped(Reported::unfinished()),
            "the browser is told what became of it, and is offered the retry",
        );
        assert!(
            latest.waiting.is_empty(),
            "and nothing is named as still to come, there being no worker left \
             to take it",
        );
    }

    // What the drop route reads to decide whether the browser has anything to
    // follow: armed counts, because arming is what the drop just did and the
    // worker has not necessarily run a line yet.
    #[test]
    fn a_freeze_is_running_from_the_moment_it_is_armed() {
        let freezes = Freezes::new();
        assert!(!freezes.running());

        freezes.arm(book());
        assert!(freezes.running());

        freezes.take_next();
        assert!(freezes.running());
        assert_eq!(freezes.take_next(), None);
        assert!(!freezes.running());
    }
}
