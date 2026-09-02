use tokio::sync::watch;

use crate::folder::Folder;

use super::progress::Progress;
use super::FreezeActivity;

/// What the server is packing into the Library, and what it packed last.
#[derive(Debug)]
pub struct Freezes {
    /// The one place any of this is written, so that every reader sees a whole
    /// answer and a case can wait on one.
    progress: watch::Sender<Progress>,
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
        }
    }

    /// The latest freeze, running or finished, and `None` where none has run.
    ///
    /// A finished one is kept rather than cleared, because the two things a
    /// browser most needs from this are things a finished freeze says: what the
    /// run left alone, and whether Storage stopped it — the state the retry is
    /// offered from.
    pub fn activity(&self) -> Option<FreezeActivity> {
        self.progress.borrow().activity.clone()
    }

    /// Whether a freeze is running or waiting to run.
    ///
    /// What the drop route asks before answering, so that a browser is told
    /// there is something to follow.
    pub fn running(&self) -> bool {
        !self.progress.borrow().settled()
    }

    /// Waits until nothing is being packed and nothing is waiting.
    ///
    /// What a case drives the work with. Arming is synchronous, so a case whose
    /// drop has landed has already put the freeze on this value by the time it
    /// awaits here — which is what lets the cases assert on a finished freeze
    /// without sleeping on one.
    pub async fn settled(&self) {
        let mut watched = self.progress.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait; a channel that closed anyway leaves nothing to wait
        // for.
        let _ = watched.wait_for(Progress::settled).await;
    }

    /// Asks for `folder` to be packed, and says whether a worker has to be
    /// started for it. See [`Progress::arm`].
    pub(super) fn arm(&self, folder: Folder) -> bool {
        let mut start = false;
        self.progress
            .send_modify(|progress| start = progress.arm(folder));
        start
    }

    /// The next folder to pack, or nothing — in which case the worker is done
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

    /// Says where the freeze in progress has got to.
    pub(super) fn publish(&self, activity: &FreezeActivity) {
        self.progress
            .send_modify(|progress| progress.activity = Some(activity.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::Freezes;
    use crate::folder::Folder;
    use crate::freeze::FreezeStatus;
    use coffret_device::EntryPath;

    fn book() -> Folder {
        Folder::named(Some(EntryPath::nfc("books/vol-1")))
    }

    // The half of a worker's leaving that `Progress` cannot state: putting the
    // state back is of no use to anyone unless the change is sent. What waits on
    // it is a case awaiting `settled` and, through the activity route, a browser
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
            "a wait for the freeze to settle is ended by this and by nothing else",
        );
        let activity = freezes
            .activity()
            .expect("a freeze that was armed is on record");
        assert_eq!(activity.status, FreezeStatus::Stopped);
        assert!(
            activity.stopped.is_some(),
            "the browser is told what became of it, and is offered the retry",
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
