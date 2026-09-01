use tokio::sync::watch;

use super::progress::Progress;
use super::SyncActivity;

/// What the server is carrying into the Library, and what it carried last.
#[derive(Debug)]
pub struct Syncs {
    /// The one place any of this is written, so that every reader sees a whole
    /// answer and a case can wait on one.
    progress: watch::Sender<Progress>,
}

impl Default for Syncs {
    fn default() -> Self {
        Self::new()
    }
}

impl Syncs {
    /// Nothing being synced.
    pub fn new() -> Self {
        Self {
            progress: watch::channel(Progress::default()).0,
        }
    }

    /// The latest sync, running or finished, and `None` where none has run.
    ///
    /// A finished one is kept rather than cleared, because the two things a
    /// browser most needs from this are things a finished sync says: what the run
    /// left alone, and whether Storage stopped it — the state the retry is
    /// offered from.
    pub fn activity(&self) -> Option<SyncActivity> {
        self.progress.borrow().activity.clone()
    }

    /// Waits until nothing is being synced and nothing is armed.
    ///
    /// What a case drives the work with. Arming is synchronous, so a case whose
    /// upload has landed has already put the sync on this value by the time it
    /// awaits here — which is what lets the cases assert on a finished sync
    /// without sleeping on one.
    pub async fn settled(&self) {
        let mut watched = self.progress.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait; a channel that closed anyway leaves nothing to wait
        // for.
        let _ = watched.wait_for(Progress::settled).await;
    }

    /// Asks for a sync, and says whether a worker has to be started for it. See
    /// [`Progress::arm`].
    pub(super) fn arm(&self) -> bool {
        let mut start = false;
        self.progress.send_modify(|progress| start = progress.arm());
        start
    }

    /// Whether there is a run to take up — and where there is not, the worker is
    /// done and stops. See [`Progress::take_next`].
    pub(super) fn take_next(&self) -> bool {
        let mut taken = false;
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

    /// Says where the sync in progress has got to.
    pub(super) fn publish(&self, activity: &SyncActivity) {
        self.progress
            .send_modify(|progress| progress.activity = Some(activity.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::Syncs;
    use crate::sync::SyncStatus;

    // The half of a worker's leaving that `Progress` cannot state: putting the
    // state back is of no use to anyone unless the change is sent. What waits on
    // it is a case awaiting `settled` and, through the activity route, a browser
    // polling for the run to end — and `send_if_modified` sends nothing at all
    // where the closure reports nothing changed, so a sync abandoned without a
    // notification is exactly the wait that never ends.
    #[test]
    fn abandoning_a_sync_tells_whoever_is_waiting_on_it() {
        let syncs = Syncs::new();
        assert!(syncs.arm());
        assert!(syncs.take_next());

        let mut watched = syncs.progress.subscribe();
        drop(watched.borrow_and_update());
        syncs.abandon();

        assert!(
            watched.has_changed().expect("the sender outlives the case"),
            "a wait for the sync to settle is ended by this and by nothing else",
        );
        let activity = syncs
            .activity()
            .expect("a sync that was armed is on record");
        assert_eq!(activity.status, SyncStatus::Stopped);
        assert!(
            activity.stopped.is_some(),
            "the browser is told what became of it, and is offered the retry",
        );
    }
}
