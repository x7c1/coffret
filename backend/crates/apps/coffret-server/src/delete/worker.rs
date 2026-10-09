use std::sync::Arc;

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
            self.0.abandon();
        }
    }
}
