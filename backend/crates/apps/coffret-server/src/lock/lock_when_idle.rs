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
/// it would spend some of the first interval — the whole of it, where somebody
/// asked for a short one — before the socket had answered anything.
///
/// It sleeps to the moment the quiet would be up rather than polling, and looks
/// again when it wakes. A request that wanted the Library while it slept moved
/// the moment, and the wait starts afresh from there; that is why this is a loop
/// and not a single sleep — an interval is "quiet since somebody last wanted
/// the Library", not "quiet since the server started".
///
/// It defers rather than interrupts. Work that is running is somebody being
/// here for the whole of it, so a piece of work that outlasts the interval
/// pushes this back and the wait starts afresh from the moment it finished;
/// what the lock ends is the next thing to ask. The moment between reading the
/// clock and emptying the cell is not fenced against a request arriving in it,
/// and does not need to be: whoever took a handle first finishes on it, and
/// nothing is torn in half (spec: DK-2).
///
/// It does not return. Once the Library is locked it waits for the next unlock
/// (spec: DK-1) — the desktop app taking the Passphrase again in its own window
/// — and is armed afresh from that moment, so a Library unlocked in place locks
/// again after the same interval of quiet. A server started from the command
/// line is never unlocked in place, and this then waits for as long as the
/// server runs, holding nothing.
pub async fn lock_when_idle(state: Arc<ServerState>, interval: Duration) {
    loop {
        // Armed: serving starts now, or the Library has just been unlocked, so
        // the quiet does too.
        state.seen();
        lock_after_quiet(&state, interval).await;
        // Counted in seconds and not named in minutes, because what is worth
        // reading afterwards is the interval that was in force rather than the
        // unit somebody typed it in.
        info!(
            operation = "lock",
            how = "idle",
            idle_seconds = interval.as_secs(),
            "nobody wanted the Library for the idle interval, so it was locked",
        );
        state.until_unlocked().await;
    }
}

/// Waits out one interval of quiet, however often somebody interrupts it, and
/// locks at the end of it.
async fn lock_after_quiet(state: &ServerState, interval: Duration) {
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
        state.lock();
        return;
    }
}
