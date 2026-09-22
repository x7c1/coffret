use tokio::sync::watch;

use crate::folder::Folder;
use crate::latest::Latest;

use super::progress::Progress;
use super::Activity;

/// What the server is bringing over, and what it has brought over last.
#[derive(Debug)]
pub struct Fills {
    /// The one place any of this is written, so that every reader sees a whole
    /// answer and a case can wait on one.
    progress: watch::Sender<Progress>,
}

impl Default for Fills {
    fn default() -> Self {
        Self::new()
    }
}

impl Fills {
    /// Nothing being filled.
    pub fn new() -> Self {
        Self {
            progress: watch::channel(Progress::default()).0,
        }
    }

    /// The latest fill, the runs it took the record from, the folders waiting
    /// behind it and the ones its queue lost — and `None` where no fill has run.
    ///
    /// A finished fill is kept rather than cleared, because the two things a
    /// browser most needs from this are things a finished fill says: which
    /// Entries were declined, and whether Storage stopped it — the state the
    /// retry is offered from. The three lists are read beside it rather than out
    /// of it, because none of them is any one run's property: the folder on
    /// record is the one being brought over, the queue is what nothing has been
    /// said about yet, what a worker that died threw away outlives the run it was
    /// queued behind, and a run Storage stopped goes on being a folder somebody
    /// asked for and did not get after the next one has taken the record from it.
    ///
    /// All four under one borrow, which is the whole reason they live in one
    /// value. They change together: a worker that dies marks its run stopped and
    /// moves the folders behind it onto the dropped list in the same stroke, a
    /// folder taken off the queue becomes the run on record and puts the stopped
    /// one it replaced onto the displaced list in another, and an arming that
    /// finds nothing running announces the run and queues the folder in a third.
    /// Read one at a time, an answer could carry half of any of them — a run
    /// still saying `filling` beside the folders that very ending threw away, or
    /// a run still saying `done` beside a queue already taken up, which is a
    /// browser told there is nothing left to follow at the moment the next run
    /// starts.
    pub fn reported(&self) -> Option<Latest<Activity>> {
        let progress = self.progress.borrow();
        let activity = progress.activity.clone()?;
        Some(Latest {
            activity,
            displaced: progress.displaced().to_vec(),
            waiting: progress.waiting(),
            dropped: progress.dropped().to_vec(),
        })
    }

    /// Waits until nothing is being filled and nothing is armed.
    ///
    /// What a case drives the work with. Arming is synchronous, so a case that
    /// has asked for a file has already put the fill on this value by the time
    /// it awaits here — which is what lets the cases assert on a finished fill
    /// without sleeping on one.
    pub async fn settled(&self) {
        let mut watched = self.progress.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait; a channel that closed anyway leaves nothing to
        // wait for.
        let _ = watched.wait_for(Progress::settled).await;
    }

    /// Makes `folder` what is filled next, and says whether a worker has to be
    /// started for it. See [`Progress::arm`].
    pub(super) fn arm(&self, folder: Folder) -> bool {
        let mut start = false;
        self.progress
            .send_modify(|progress| start = progress.arm(folder));
        start
    }

    /// Puts `folder` at the back of the queue, and says whether a worker has to
    /// be started for it. See [`Progress::queue`].
    pub(super) fn queue(&self, folder: Folder) -> bool {
        let mut start = false;
        self.progress
            .send_modify(|progress| start = progress.queue(folder));
        start
    }

    /// The next folder to fill, or nothing — in which case the worker is done
    /// and stops. See [`Progress::take_next`].
    pub(super) fn take_next(&self) -> Option<Folder> {
        let mut taken = None;
        self.progress
            .send_modify(|progress| taken = progress.take_next());
        taken
    }

    /// Puts back what a worker that ended without taking its leave left set.
    ///
    /// Notified only where there was something to put back, which is why this
    /// goes through [`send_if_modified`](watch::Sender::send_if_modified): the
    /// ordinary ending has cleared all of it already.
    pub(super) fn abandon(&self) {
        self.progress.send_if_modified(Progress::abandon);
    }

    /// Whether another folder has been armed under the fill in progress.
    pub(super) fn superseded(&self) -> bool {
        self.progress.borrow().superseded()
    }

    /// Says where the fill in progress has got to.
    ///
    /// The run number is stamped on here rather than carried by the caller: the
    /// value a run builds is its own account of one folder, and which run of the
    /// flow that is is the queue's to say.
    pub(super) fn publish(&self, activity: &Activity) {
        self.progress.send_modify(|progress| {
            progress.activity = Some(Activity {
                run: progress.run(),
                ..activity.clone()
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::Fills;
    use crate::fill::FillStatus;
    use crate::folder::Folder;

    use crate::entry_paths::entry_path;

    fn albums() -> Folder {
        Folder::named(Some(entry_path("albums")))
    }

    // The half of a worker's leaving that `Progress` cannot state: putting the
    // state back is of no use to anyone unless the change is sent. What waits on
    // it is a case awaiting `settled` and, through the activity route, a
    // browser polling a count — and `send_if_modified` sends nothing at all
    // where the closure reports nothing changed, so a fill abandoned without a
    // notification is exactly the wait that never ends.
    #[test]
    fn abandoning_a_fill_tells_whoever_is_waiting_on_it() {
        let fills = Fills::new();
        assert!(fills.arm(albums()));
        assert_eq!(fills.take_next(), Some(albums()));

        let mut watched = fills.progress.subscribe();
        drop(watched.borrow_and_update());
        fills.abandon();

        assert!(
            watched.has_changed().expect("the sender outlives the case"),
            "a wait for the fill to settle is ended by this and by nothing else",
        );
        let latest = fills
            .reported()
            .expect("a fill that was armed is on record");
        assert_eq!(latest.activity.status, FillStatus::Stopped);
        assert!(
            latest.activity.stopped.is_some(),
            "the browser is told what became of it, and is offered the retry",
        );
        assert!(
            latest.waiting.is_empty(),
            "and nothing is named as still to come, there being no worker left \
             to take it",
        );
    }
}
