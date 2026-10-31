use std::collections::BTreeSet;
use std::sync::Arc;

use coffret_device::Step;
use tokio::sync::watch;

use crate::activity_log::{ActivityLog, Ended};
use crate::adds::AddId;

use super::progress::Progress;
use super::SyncRun;

/// What the server is carrying into the Library, and what it carried last.
#[derive(Debug)]
pub struct Syncs {
    /// The one place any of this is written, so that every reader sees a whole
    /// answer and a case can wait on one.
    progress: watch::Sender<Progress>,
    /// Where every run that ends is logged, shared with the rest of the
    /// server's work (see [`ActivityLog`]).
    log: Arc<ActivityLog>,
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
            log: Arc::new(ActivityLog::new()),
        }
    }

    /// Nothing being synced, logging what ends into `log` — the server's one
    /// log of its finished work, which [`new`](Self::new) keeps apart for a
    /// case that drives this flow alone.
    pub fn logging_to(log: Arc<ActivityLog>) -> Self {
        Self { log, ..Self::new() }
    }

    /// The latest sync, running or finished, and `None` where none has run.
    ///
    /// A finished one is kept rather than cleared, because the two things a
    /// browser most needs from this are things a finished sync says: what the run
    /// left alone, and whether Storage stopped it — the state the retry is
    /// offered from.
    pub fn reported(&self) -> Option<SyncRun> {
        self.progress.borrow().on_record.clone()
    }

    /// Waits until nothing is being synced and nothing is armed.
    ///
    /// What a case drives the work with. Arming is synchronous, so a case whose
    /// upload has landed has already put the sync on this value by the time it
    /// awaits here — which is what lets the cases assert on a finished sync
    /// without sleeping on one.
    pub async fn until_idle(&self) {
        let mut watched = self.progress.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait; a channel that closed anyway leaves nothing to wait
        // for.
        let _ = watched.wait_for(Progress::idle).await;
    }

    /// Asks for a sync, and says whether a worker has to be started for it. See
    /// [`Progress::arm`].
    pub(super) fn arm(&self, add: Option<AddId>) -> bool {
        let mut start = false;
        self.progress
            .send_modify(|progress| start = progress.arm(add));
        start
    }

    /// Asks the sync on record to stop, where it is running and has not begun
    /// committing (spec: LA-35).
    ///
    /// Asked for every add a person stops that a sync is to carry, whichever
    /// run it is armed for: the run under way may walk the folder the add
    /// wrote into, and so be about to read the files being taken away. The
    /// adds such a run carried that nobody stopped are armed again once it has
    /// stopped.
    pub(crate) fn ask_stop(&self) {
        self.progress.send_if_modified(|progress| {
            progress.on_record.as_mut().is_some_and(|run| {
                matches!(run.status, super::SyncStatus::Syncing) && run.stop.ask()
            })
        });
    }

    /// The adds run `run` carries, where it is the sync on record and still
    /// running: what a Stop pressed on its line stops.
    pub(crate) fn carried_by(&self, run: u64) -> Option<BTreeSet<AddId>> {
        let progress = self.progress.borrow();
        let on_record = progress.on_record.as_ref()?;
        (on_record.run == run && matches!(on_record.status, super::SyncStatus::Syncing))
            .then(|| on_record.adds.clone())
    }

    /// The adds the run on record carries, and whether it was requested too —
    /// what the run just taken up reads as it starts.
    pub(super) fn carried(&self) -> (BTreeSet<AddId>, bool) {
        let progress = self.progress.borrow();
        progress
            .on_record
            .as_ref()
            .map(|run| (run.adds.clone(), run.requested))
            .unwrap_or_default()
    }

    /// Takes into the run on record what was armed while it waited for its
    /// turn. See [`Progress::absorb`].
    pub(super) fn absorb(&self) {
        self.progress.send_if_modified(Progress::absorb);
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

    /// Arms again what a stopped run carried that nobody stopped. See
    /// [`Progress::rearm`].
    pub(super) fn rearm(&self, adds: BTreeSet<AddId>, requested: bool) {
        self.progress
            .send_modify(|progress| progress.rearm(adds, requested));
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
    /// For a worker that panicked, and for no other: the ordinary ending clears
    /// all of this itself, and by the time it is over another worker may own
    /// the flag. Notified only where there was something to put back, which is
    /// why this goes through [`send_if_modified`](watch::Sender::send_if_modified).
    ///
    /// A run it stops is logged as one that stopped through
    /// [`publish`](Self::publish) is.
    pub(super) fn abandon(&self) {
        self.progress.send_if_modified(|progress| {
            let running = progress.is_syncing();
            let modified = progress.abandon();
            if modified && running {
                if let Some(run) = &progress.on_record {
                    self.log.record(Ended::Sync(run.clone()));
                }
            }
            modified
        });
    }

    /// Says where the sync in progress has got to.
    ///
    /// The run number is stamped on here rather than carried by the caller: the
    /// value a run builds is its own account of one walk, and which run of the
    /// flow that is is this value's to say.
    pub(super) fn publish(&self, run: &SyncRun) {
        self.progress.send_modify(|progress| {
            let published = SyncRun {
                run: progress.run(),
                ..run.clone()
            };
            // Logged under the same lock that publishes it, so no reader sees
            // the run over before the log has it. A run a person stopped is
            // not: it is stopped only through the adds it carried, and each of
            // those is logged as what it came to (spec: LA-12, LA-36).
            if !matches!(
                published.status,
                super::SyncStatus::Syncing | super::SyncStatus::Cancelled
            ) {
                self.log.record(Ended::Sync(published.clone()));
            }
            progress.on_record = Some(published);
        });
    }

    /// Says how far into the running sync the flow has got.
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
}

#[cfg(test)]
mod tests {
    use super::Syncs;
    use crate::reported::Reported;
    use crate::sync::SyncStatus;

    // The half of a worker's leaving that `Progress` cannot state: putting the
    // state back is of no use to anyone unless the change is sent. What waits on
    // it is a case awaiting `until_idle` and, through the work route, a browser
    // polling for the run to end — and `send_if_modified` sends nothing at all
    // where the closure reports nothing changed, so a sync abandoned without a
    // notification is exactly the wait that never ends.
    #[test]
    fn abandoning_a_sync_tells_whoever_is_waiting_on_it() {
        let syncs = Syncs::new();
        assert!(syncs.arm(None));
        assert!(syncs.take_next());

        let mut watched = syncs.progress.subscribe();
        drop(watched.borrow_and_update());
        syncs.abandon();

        assert!(
            watched.has_changed().expect("the sender outlives the case"),
            "a wait for the sync to finish is ended by this and by nothing else",
        );
        let run = syncs
            .reported()
            .expect("a sync that was armed is on record");
        assert_eq!(
            run.status,
            SyncStatus::Stopped(Reported::unfinished()),
            "the browser is told what became of it, and is offered the retry",
        );
    }
}
