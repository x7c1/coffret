use std::sync::Arc;

use crate::state::ServerState;

use super::run;
use super::Fills;

/// The one worker, taking folders until none is armed.
pub(super) async fn work(state: Arc<ServerState>) {
    // Whichever way this ends, what says a worker is running is put back. It
    // ends by finding nothing armed, which puts it back already — and it ends by
    // panicking, which without this would leave the flag set with nothing behind
    // it: no arming would start another worker for the rest of the process, and
    // the run on record would go on saying `filling` to a browser that polls it.
    let _leaving = Leaving(&state.fills);
    while let Some(folder) = state.fills.take_next() {
        run::fill(&state, &folder).await;
    }
}

/// The worker's leaving, whether it meant to or not.
///
/// A guard rather than a line at the end of [`work`], because the end of `work`
/// is the one ending that needs no putting back: it is the other one — the job
/// panicking, which unwinds past every line there is — that this exists for.
///
/// So it puts back only while unwinding from a panic. By the time an ordinary
/// ending drops this, `take_next` has already said the worker is gone, and an
/// arming in between may have started the next one: putting back then would
/// take that new worker's run for this one's leftovers — discarding the folder
/// it was armed for and reporting it stopped. The worker is spawned and never
/// aborted, so a panic is the only ending that does not go through `take_next`.
struct Leaving<'a>(&'a Fills);

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

    use super::{Fills, Leaving};
    use crate::entry_paths::entry_path;
    use crate::fill::FillStatus;
    use crate::folder::Folder;
    use crate::reported::Reported;

    fn albums() -> Folder {
        Folder::named(Some(entry_path("albums")))
    }

    fn photos() -> Folder {
        Folder::named(Some(entry_path("photos")))
    }

    // A worker that has found nothing armed has left, and a fetch arming a fill
    // before its guard drops starts the next worker: the old guard dropping
    // afterwards is not that new run dying, and must leave its folder armed.
    #[test]
    fn a_worker_leaving_the_ordinary_way_leaves_the_next_run_alone() {
        let fills = Fills::new();
        assert!(fills.arm(albums(), None));
        let leaving = Leaving(&fills);
        assert_eq!(fills.take_next(), Some(albums()));
        assert_eq!(fills.take_next(), None);

        assert!(
            fills.arm(photos(), None),
            "nothing is running, so the arming starts a worker of its own",
        );
        drop(leaving);

        let latest = fills.reported().expect("an armed fill is on record");
        assert!(
            latest.discarded.is_empty(),
            "and nothing is thrown away by a worker that did not die",
        );
        assert_ne!(
            latest.on_record.status,
            FillStatus::Stopped(Reported::unfinished()),
            "nor is the new run reported stopped",
        );
        assert_eq!(
            fills.take_next(),
            Some(photos()),
            "the new arming is still armed for the worker it started",
        );
    }

    // The ending the guard exists for: a panic unwinding past the worker puts
    // back what it left set.
    #[test]
    fn a_worker_that_panics_is_put_back() {
        let fills = Fills::new();
        assert!(fills.arm(albums(), None));

        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let _leaving = Leaving(&fills);
            assert_eq!(fills.take_next(), Some(albums()));
            panic!("the job panics mid-fill");
        }));
        assert!(unwound.is_err());

        let latest = fills.reported().expect("an armed fill is on record");
        assert_eq!(
            latest.on_record.status,
            FillStatus::Stopped(Reported::unfinished()),
        );
        assert!(
            fills.arm(albums(), None),
            "and the next arming starts a worker again",
        );
    }
}
