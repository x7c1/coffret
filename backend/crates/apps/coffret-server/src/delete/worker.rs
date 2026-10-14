use std::sync::Arc;

use tracing::warn;

use crate::state::ServerState;

use super::run;
use super::Deletes;

/// The one worker, deleting until nothing is waiting.
pub(super) async fn work(state: Arc<ServerState>) {
    // Whichever way this ends, what says a worker is running is put back. The
    // ordinary ending — nothing waiting — puts it back already; a panic would
    // otherwise leave it set with nothing behind it, so no later deletion would
    // ever start and the run on record would go on saying `deleting`.
    let _leaving = Leaving(&state.deletes);
    while let Some(target) = state.deletes.take_next() {
        run::delete(&state, target).await;
    }
}

/// The worker's leaving, whether it meant to or not.
///
/// It puts back only while unwinding from a panic, for the reason the freeze's
/// worker gives: by the time an ordinary ending drops this, `take_next` has
/// already said the worker is gone and a deletion armed in between may have
/// started the next one.
struct Leaving<'a>(&'a Deletes);

impl Drop for Leaving<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            // Said here, beside the state it puts back, because the panic itself
            // reaches nothing but the default hook's line on standard error, and
            // the log is what a person reads to tell one failure from another.
            warn!(
                operation = "delete",
                "the delete worker panicked; the deletion it was running is put back as stopped",
            );
            self.0.abandon();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::{Deletes, Leaving};
    use crate::delete::Target;
    use crate::entry_paths::entry_path;

    fn albums() -> Target {
        Target {
            folder: Some(entry_path("albums")),
            paths: BTreeSet::new(),
        }
    }

    // The panic is in the log beside the run it put back, under the worker's
    // own operation, so a person reading it can tell the stopped run from one
    // Storage refused.
    #[test]
    fn a_worker_that_panics_says_so_in_the_log() {
        let logs = coffret_logging::testing::CapturedLogs::capture();
        let deletes = Deletes::new();
        assert!(deletes.arm(albums()));

        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let _leaving = Leaving(&deletes);
            assert_eq!(deletes.take_next(), Some(albums()));
            panic!("the job panics");
        }));
        assert!(unwound.is_err());

        let event = logs.only(tracing::Level::WARN);
        assert_eq!(event.field("operation"), "delete");
    }
}
