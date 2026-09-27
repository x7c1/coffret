//! Driving the background runs, and waiting for them rather than sleeping on
//! them.

use std::sync::Arc;
use std::time::Duration;

use coffret_server::{fill_folder, freeze_folder, lock_when_idle, queue_folder, Folder};
use tokio::task::JoinHandle;

use super::{entry_path, Served};

impl Served {
    /// Waits for the background sync to finish, whatever it came to.
    ///
    /// No sleep, and no polling: an upload arms the sync before it answers, so a
    /// case whose files have landed has already put the run on the state it waits
    /// on here.
    pub async fn sync_idle(&self) {
        self.state.syncs.until_idle().await;
    }

    /// Arms a fill without going through a route.
    ///
    /// Two of these back to back, with nothing awaited in between, is a fill
    /// superseded before it began — the one way to state "latest wins" as a
    /// case, since anything that awaits gives the worker a chance to run and
    /// leaves what it managed first up to the scheduler.
    pub fn arm_fill(&self, folder: &str) {
        let named = (!folder.is_empty()).then(|| entry_path(folder));
        fill_folder(Arc::clone(&self.state), Folder::named(named));
    }

    /// Asks for a fill by name without going through a route.
    ///
    /// Two of these back to back, with nothing awaited in between, is a second
    /// folder asked for while the first is still being brought over — the one
    /// way to state "a button waits its turn" as a case, since anything that
    /// awaits gives the worker a chance to finish and leaves the ordering up to
    /// the scheduler.
    pub fn queue_fill(&self, folder: &str) {
        let named = (!folder.is_empty()).then(|| entry_path(folder));
        queue_folder(Arc::clone(&self.state), Folder::named(named));
    }

    /// Waits for the background fill to finish, whatever it came to.
    ///
    /// No sleep, and no polling: arming is synchronous, so a case that has asked
    /// for a file has already put the fill on the state it waits on here.
    pub async fn fill_idle(&self) {
        self.state.fills.until_idle().await;
    }

    /// Arms a freeze without going through a route.
    ///
    /// Two of these back to back, with nothing awaited in between, is a second
    /// book asked for while the first is still being packed — the one way to
    /// state "it waits its turn" as a case, since anything that awaits gives the
    /// worker a chance to finish and leaves the ordering up to the scheduler.
    pub fn arm_freeze(&self, folder: &str) {
        let named = (!folder.is_empty()).then(|| entry_path(folder));
        freeze_folder(Arc::clone(&self.state), Folder::named(named));
    }

    /// Waits for the background freeze to finish, whatever it came to.
    ///
    /// No sleep, and no polling: a book drop arms the freeze before it answers,
    /// so a case whose pages have landed has already put the run on the state it
    /// waits on here.
    pub async fn freeze_idle(&self) {
        self.state.freezes.until_idle().await;
    }

    /// Watches for the idle interval, as the binary does beside the socket.
    ///
    /// The clock is the case's own: every case over this runs with time paused,
    /// so a quarter of an hour of quiet is stated rather than spent. The yield
    /// is what puts the watcher on its first sleep before the case moves the
    /// clock — without it the first advance would be one nothing was waiting on.
    ///
    /// The handle is what a case asking whether the watcher is still there
    /// reads: a task that panicked is a finished task, and a watcher that had
    /// panicked would leave a Library that stays open and a case that could not
    /// tell that from one being kept open on purpose.
    pub async fn watch_idle(&self, after: Duration) -> JoinHandle<()> {
        let watcher = tokio::spawn(lock_when_idle(Arc::clone(&self.state), after));
        tokio::task::yield_now().await;
        watcher
    }
}
