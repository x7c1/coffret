use std::sync::Arc;

use crate::state::ServerState;

use super::run;

/// The one background task, running syncs until none is armed.
pub(super) async fn work(state: Arc<ServerState>) {
    // Whichever way this ends, what says a worker is running is put back. It ends
    // by finding nothing armed, which puts it back already — and it ends by
    // panicking, which without this would leave the flag set with nothing behind
    // it: no drop would start another worker for the rest of the process, and the
    // activity would go on saying `syncing` to a browser that polls it.
    let _leaving = Leaving(Arc::clone(&state));
    while state.syncs.take_next() {
        run::sync(&state).await;
    }
}

/// The worker's leaving, whether it meant to or not.
///
/// A guard rather than a line at the end of [`work`], because the end of `work`
/// is the one ending that needs no putting back: it is the other one — the job
/// panicking, which unwinds past every line there is — that this exists for.
struct Leaving(Arc<ServerState>);

impl Drop for Leaving {
    fn drop(&mut self) {
        self.0.syncs.abandon();
    }
}
