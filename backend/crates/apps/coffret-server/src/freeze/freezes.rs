use std::collections::BTreeSet;
use std::sync::Arc;

use coffret_device::Step;
use tokio::sync::watch;

use crate::activity_log::{ActivityLog, Ended, Queue};
use crate::adds::AddId;
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
            stopping: progress.stopping(),
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
            // the run over before the log has it. A run a person stopped
            // through the adds it carried is logged as those adds, each as
            // what it came to, and not as itself — unless a Stop on its line
            // also took away a request of its book's own (a folder somebody
            // asked to have packed, or one still owed a pack since a run of it
            // stopped): that request is logged `cancelled` beside the adds, so
            // whoever asked sees what became of it (spec: LA-12, LA-36).
            let through_adds = matches!(published.status, super::FreezeStatus::Cancelled)
                && !published.adds.is_empty()
                && !progress.withdrawn();
            if !matches!(published.status, super::FreezeStatus::Freezing) && !through_adds {
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
                // Under the same lock a stop is asked under, so a run is never
                // both committing and taking a stop (spec: LA-35).
                run.stop.hear(&step);
                run.step = Some(step);
            }
        });
    }

    /// Asks the freeze on record to stop, where it is still freezing — under
    /// way, or announced and not yet taken up — and has not begun committing
    /// (spec: LA-35).
    pub(crate) fn ask_stop(&self) {
        self.ask(false);
    }

    /// Asks the freeze on record to stop as [`ask_stop`](Self::ask_stop) does,
    /// and where the ask is taken, stops the request of its book's own along
    /// with it: what a Stop on the run's line asks (spec: LA-36). See
    /// [`Progress::withdraw`].
    pub(crate) fn ask_stop_whole(&self) {
        self.ask(true);
    }

    fn ask(&self, whole: bool) {
        self.progress
            .send_if_modified(|progress| Self::ask_in(progress, whole));
    }

    /// Asks the run on record to stop, under the lock its caller holds, and
    /// says whether the ask was taken.
    fn ask_in(progress: &mut Progress, whole: bool) -> bool {
        let taken = progress.on_record.as_mut().is_some_and(|run| {
            matches!(run.status, super::FreezeStatus::Freezing) && run.stop.ask()
        });
        if taken && whole {
            progress.withdraw();
        }
        taken
    }

    /// Whether the book the worker is on is still asked for by something
    /// besides its adds. See [`Book::requested`].
    pub(super) fn requested(&self) -> bool {
        self.progress.borrow().requested()
    }

    /// The adds the book of `folder` waiting its turn holds, and `None` where
    /// no book of that folder waits behind the run on record.
    pub(crate) fn waiting_adds(&self, folder: &str) -> Option<BTreeSet<AddId>> {
        self.progress.borrow().waiting_adds(folder)
    }

    /// Takes the book of `folder` off the queue, where it waits holding no
    /// add, and logs it `cancelled`, a run stopped before it began
    /// (spec: LA-12, LA-34).
    pub(crate) fn cancel_waiting(&self, folder: &str) {
        self.progress.send_if_modified(|progress| {
            let Some(book) = progress.cancel_waiting(folder) else {
                return false;
            };
            self.log.record(Ended::Freeze(FreezeRun {
                status: super::FreezeStatus::Cancelled,
                ..FreezeRun::starting(book)
            }));
            true
        });
    }

    /// Stops the book of `folder` that was waiting its turn holding `adds`, as
    /// a Stop on its line does: marks it stopped where it still waits (see
    /// [`Progress::stop_waiting`]), and where the worker took it up since
    /// `adds` were read, so that the run on record carries them now, asks that
    /// run to stop as a Stop on its own line does (see
    /// [`ask_stop_whole`](Self::ask_stop_whole)). Either way a request of the
    /// book's own is withdrawn with its adds: the folder is not armed again
    /// once they are undone (spec: LA-36), and the run is logged as a
    /// `cancelled` freeze of its folder beside them (spec: LA-12).
    ///
    /// Under one lock, so the book cannot be taken up between finding it no
    /// longer waiting and asking the run that took it.
    pub(crate) fn stop_queued(&self, folder: &str, adds: &BTreeSet<AddId>) {
        self.progress.send_if_modified(|progress| {
            if progress.stop_waiting(folder) {
                return true;
            }
            let carried = progress.on_record.as_ref().is_some_and(|run| {
                matches!(run.status, super::FreezeStatus::Freezing)
                    && adds.iter().any(|id| run.adds.contains(id))
            });
            carried && Self::ask_in(progress, true)
        });
    }

    /// The adds run `run` carries, where it is the freeze on record and still
    /// freezing: what a Stop pressed on its line stops along with it.
    pub(crate) fn carried_by(&self, run: u64) -> Option<BTreeSet<AddId>> {
        let progress = self.progress.borrow();
        let on_record = progress.on_record.as_ref()?;
        (on_record.run == run && matches!(on_record.status, super::FreezeStatus::Freezing))
            .then(|| on_record.adds.clone())
    }

    /// Whether the freeze on record is freezing and carries the add `id`: one
    /// whose stop is that run's to answer now, rather than the run's its book
    /// waits for.
    pub(crate) fn carries(&self, id: &AddId) -> bool {
        self.progress
            .borrow()
            .on_record
            .as_ref()
            .is_some_and(|run| {
                matches!(run.status, super::FreezeStatus::Freezing) && run.adds.contains(id)
            })
    }

    /// Whether the run on record was asked to stop and has not answered.
    pub(super) fn asked(&self) -> bool {
        self.progress
            .borrow()
            .on_record
            .as_ref()
            .is_some_and(|run| run.stop.asked())
    }

    /// Waits until the run on record is asked to stop, and for ever where it
    /// is not: one arm of the race the run runs its flow in (see
    /// [`run`](super::run)).
    pub(super) async fn until_asked(&self) {
        let mut watched = self.progress.subscribe();
        let asked = watched
            .wait_for(|progress| {
                progress
                    .on_record
                    .as_ref()
                    .is_some_and(|run| run.stop.asked())
            })
            .await
            .is_ok();
        if !asked {
            // The sender is the state's own, so it outlives the run; a channel
            // that closed anyway asks nothing of it.
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Freezes;
    use crate::adds::AddId;
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

    // LA-36, LA-12: a book holding an add, stopped on its line in the queue
    // just as the worker takes it up — after its adds were read and before it
    // was marked — is stopped as its run is by a Stop on that run's line: the
    // request of the book's own is withdrawn with the add, so the folder is
    // not armed again and the run is logged `cancelled` beside the add.
    #[test]
    fn a_queued_book_taken_up_as_it_is_stopped_is_stopped_whole() {
        let freezes = Freezes::new();
        let add = AddId::named("drop-q").expect("a page's name");
        let waiting = Book::whole(Folder::named(Some(entry_path("books/vol-2")))).carrying(add);
        assert!(freezes.arm(book()));
        assert_eq!(freezes.take_next(), Some(book()));
        assert!(!freezes.arm(waiting.clone()), "queued behind the first");

        let adds = freezes
            .waiting_adds("books/vol-2")
            .expect("the book waits its turn");
        freezes.progress.send_modify(|progress| {
            if let Some(run) = progress.on_record.as_mut() {
                run.status = FreezeStatus::Done;
            }
        });
        assert_eq!(freezes.take_next(), Some(waiting), "taken up meanwhile");
        freezes.stop_queued("books/vol-2", &adds);

        assert!(freezes.asked(), "the run carrying the add is asked to stop");
        assert!(!freezes.requested(), "the book's own request is stopped");
        assert!(
            freezes.progress.borrow().withdrawn(),
            "and its run is logged cancelled beside the add",
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
