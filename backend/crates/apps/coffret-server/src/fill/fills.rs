use tokio::sync::watch;

use super::progress::Progress;
use super::{Activity, Folder};

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

    /// The latest fill, running or finished, and `None` where none has run.
    ///
    /// A finished one is kept rather than cleared, because the two things a
    /// browser most needs from this are things a finished fill says: which
    /// Entries were declined, and whether Storage stopped it — the state the
    /// retry is offered from.
    pub fn activity(&self) -> Option<Activity> {
        self.progress.borrow().activity.clone()
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
    pub(super) fn publish(&self, activity: &Activity) {
        self.progress
            .send_modify(|progress| progress.activity = Some(activity.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::Fills;
    use crate::fill::{FillStatus, Folder};
    use coffret_device::EntryPath;

    fn albums() -> Folder {
        Folder::named(Some(EntryPath::nfc("albums")))
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
        let activity = fills
            .activity()
            .expect("a fill that was armed is on record");
        assert_eq!(activity.status, FillStatus::Stopped);
        assert!(
            activity.stopped.is_some(),
            "the browser is told what became of it, and is offered the retry",
        );
    }
}
