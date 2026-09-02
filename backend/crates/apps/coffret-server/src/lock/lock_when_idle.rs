use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep_until;
use tracing::info;

use crate::state::ServerState;

/// Locks the Library once nobody has wanted it for `interval` (spec: DK-4).
///
/// One task, started beside the socket and holding no key of its own: what it
/// holds is the state, and the keys are behind the custody cell inside it — so
/// the wiping that a lock does is not held up by this task still existing.
///
/// The interval is counted from here rather than from when the state was built.
/// What comes before this task is the unlock and the catalog catching up with
/// the Library, which together can be a minute of a Storage answering slowly,
/// and none of it is time anybody could have been at the keyboard for. Counting
/// it would spend part of the first interval — the whole of it, where somebody
/// asked for a short one — before the socket had answered anything.
///
/// It sleeps to the moment the quiet would be up rather than polling, and looks
/// again when it wakes. A request that wanted the Library while it slept moved
/// the moment, and the wait starts afresh from there; that is why this is a loop
/// and not a single sleep — an interval is "quiet since somebody last wanted
/// the Library", not "quiet since the server started".
///
/// It never fires in the middle of an operation. Work that is running is
/// somebody being here for the whole of it, so a piece of work that outlasts
/// the interval defers this rather than meeting it, and the wait starts afresh
/// from the moment it finished. What the lock ends is the next thing to ask —
/// and even were one to land mid-operation, as the explicit lock may, whoever
/// took a handle before it finishes with it (spec: DK-2).
///
/// It returns once the Library is locked, whichever of the two locks got there
/// first. There is nothing left for it to watch: this server has no way back to
/// unlocked.
pub async fn lock_when_idle(state: Arc<ServerState>, interval: Duration) {
    // Serving starts now, so the quiet does too.
    state.seen();
    loop {
        let quiet_since = state.last_seen();
        match quiet_since.checked_add(interval) {
            Some(deadline) => sleep_until(deadline).await,
            // An interval that runs off the end of the clock. The binary
            // saturates the minutes into seconds so that a wildly large number
            // stays a wildly long wait instead of wrapping into a tiny one, and
            // this is the other end of that promise: such a wait is one that
            // simply never comes up, rather than a sum that panics this task out
            // of existence and leaves a server nothing will ever lock.
            None => std::future::pending::<()>().await,
        }
        if state.last_seen() > quiet_since {
            // Somebody wanted the Library while this slept. They are here, and
            // the interval is measured from them rather than from whoever was
            // last here before them.
            continue;
        }
        if state.lock() {
            // Counted in seconds and not named in minutes, because what is
            // worth reading afterwards is the interval that was in force rather
            // than the unit somebody typed it in.
            info!(
                operation = "lock",
                how = "idle",
                idle_seconds = interval.as_secs(),
                "nobody wanted the Library for the idle interval, so it was locked",
            );
        }
        return;
    }
}
