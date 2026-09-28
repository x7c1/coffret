use std::sync::Arc;

use crate::state::ServerState;

use super::run;
use super::Syncs;

/// The one worker, running syncs until none is armed.
pub(super) async fn work(state: Arc<ServerState>) {
    // Whichever way this ends, what says a worker is running is put back. It
    // ends by finding nothing armed, which puts it back already — and it ends by
    // panicking, which without this would leave the flag set with nothing behind
    // it: no drop would start another worker for the rest of the process, and
    // the run on record would go on saying `syncing` to a browser that polls it.
    let _leaving = Leaving(&state.syncs);
    while state.syncs.take_next() {
        run::sync(&state).await;
    }
}

/// The worker's leaving, whether it meant to or not.
///
/// A guard rather than a line at the end of [`work`], because the end of `work`
/// is the one ending that needs no putting back: it is the other one — the job
/// panicking, which unwinds past every line there is — that this exists for.
///
/// So it puts back only while unwinding from a panic. By the time an ordinary
/// ending drops this, `take_next` has already said the worker is gone, and a
/// drop in between may have started the next one: putting back then would take
/// that new worker's run for this one's leftovers — disarming the sync it was
/// started for and reporting it stopped. The worker is spawned and never
/// aborted, so a panic is the only ending that does not go through `take_next`.
struct Leaving<'a>(&'a Syncs);

impl Drop for Leaving<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.abandon();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::{Leaving, Syncs};
    use crate::reported::Reported;
    use crate::sync::SyncStatus;

    // A worker that has found nothing armed has left, and a drop arming a sync
    // before its guard goes starts the next worker: the old guard going
    // afterwards is not that new run dying, and must leave its sync armed.
    #[test]
    fn a_worker_leaving_the_ordinary_way_leaves_the_next_run_alone() {
        let syncs = Syncs::new();
        assert!(syncs.arm());
        let leaving = Leaving(&syncs);
        assert!(syncs.take_next());
        assert!(!syncs.take_next());

        assert!(
            syncs.arm(),
            "nothing is running, so the arming starts a worker of its own",
        );
        drop(leaving);

        let latest = syncs.reported().expect("an armed sync is on record");
        assert_eq!(
            latest.status,
            SyncStatus::Syncing,
            "the new run is not reported stopped by a worker that did not die",
        );
        assert!(
            syncs.take_next(),
            "the new arming is still armed for the worker it started",
        );
    }

    // The ending the guard exists for: a panic unwinding past the worker puts
    // back what it left set.
    #[test]
    fn a_worker_that_panics_is_put_back() {
        let syncs = Syncs::new();
        assert!(syncs.arm());

        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let _leaving = Leaving(&syncs);
            assert!(syncs.take_next());
            panic!("the job panics mid-sync");
        }));
        assert!(unwound.is_err());

        let latest = syncs.reported().expect("an armed sync is on record");
        assert_eq!(latest.status, SyncStatus::Stopped(Reported::unfinished()));
        assert!(syncs.arm(), "and the next arming starts a worker again");
    }
}
