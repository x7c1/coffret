use std::sync::Arc;

use tracing::warn;

use crate::state::ServerState;

use super::run;
use super::Moves;

/// The one worker, moving until nothing is pending.
pub(super) async fn work(state: Arc<ServerState>) {
    // Whichever way this ends, what says a worker is running is put back. The
    // ordinary ending — nothing pending — puts it back already; a panic would
    // otherwise leave it set with nothing behind it, so no later move would
    // ever start and every pending one would go on being drawn as moving.
    let _leaving = Leaving(&state.moves);
    while let Some(pending) = state.moves.take_next() {
        run::move_pending(&state, pending).await;
    }
}

/// The worker's leaving, whether it meant to or not.
///
/// It puts back only while unwinding from a panic, for the reason the
/// deletion's worker gives: by the time an ordinary ending drops this,
/// `take_next` has already said the worker is gone and a move accepted in
/// between may have started the next one.
struct Leaving<'a>(&'a Moves);

impl Drop for Leaving<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            // Said here, beside the state it puts back, because the panic itself
            // reaches nothing but the default hook's line on standard error.
            warn!(
                operation = "move",
                "the move worker panicked; every pending move is put back as stopped",
            );
            self.0.abandon();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use coffret_device::RenamePair;

    use super::{Leaving, Moves};
    use crate::entry_paths::entry_path;

    // The panic is in the log beside the moves it put back, under the worker's
    // own operation, and nothing is pending once it has.
    #[test]
    fn a_worker_that_panics_says_so_and_leaves_nothing_pending() {
        let logs = coffret_logging::testing::CapturedLogs::capture();
        let moves = Moves::new();
        assert!(moves
            .accept(vec![RenamePair::new(
                entry_path("albums/2026"),
                entry_path("books/2026"),
            )])
            .is_ok());

        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let _leaving = Leaving(&moves);
            assert!(moves.take_next().is_some());
            panic!("the job panics");
        }));
        assert!(unwound.is_err());

        let event = logs.only(tracing::Level::WARN);
        assert_eq!(event.field("operation"), "move");
        assert!(moves.listed().is_empty());
    }
}
