use std::any::Any;
use std::sync::{Arc, Mutex};

use coffret_usecase::fetch::EntryFetchOutcome;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::error;

use crate::error::Result;

/// What a reader's fetch is answered with: what the run said, or the panic
/// that ended it before it said anything.
pub(super) enum Answered {
    Ran(Result<EntryFetchOutcome>),
    Panicked(Box<dyn Any + Send>),
}

/// Where a reader's fetch is answered from: the moment its Entry is published,
/// the run's own end where it never is, or the watcher where the run panicked
/// first.
///
/// Whichever comes first takes the sender, so the reader is answered once.
pub(super) type Answer = Mutex<Option<oneshot::Sender<Answered>>>;

/// Takes the one sender, where nobody has yet.
pub(super) fn take(answer: &Answer) -> Option<oneshot::Sender<Answered>> {
    // A send is all anybody does with what is behind it, so a panic there left
    // the sender or nothing, and either is still the answer's state.
    answer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

/// Waits for the run of a reader's fetch to end, and says so where it ended in
/// a panic.
///
/// A run answered at its Entry's publication goes on with nobody holding its
/// handle, so a panic in the rest of it would otherwise reach nothing but the
/// default hook's line on standard error, and the log a person reads to tell
/// one failure from another would say nothing. The turns it held were given
/// back as it unwound, so the next request for the Entry runs; what is said
/// here is that this one did not finish. The Entry Path stays out of it, and
/// so does the panic's own message, which nothing composed for a log
/// (spec: EL-1, EL-5).
///
/// A run that panicked before anybody was answered answers its reader with the
/// panic, which is the account of it there.
pub(super) async fn watch(run: JoinHandle<()>, answer: Arc<Answer>) {
    let Err(failed) = run.await else {
        return;
    };
    // The run is never aborted, so a failure that is not a panic is the runtime
    // shutting down underneath it, and there is nobody left to tell.
    if !failed.is_panic() {
        return;
    }
    let payload = failed.into_panic();
    match take(&answer) {
        Some(sender) => {
            if sender.send(Answered::Panicked(payload)).is_err() {
                error!(
                    operation = "fetch_entry",
                    "a parcel read panicked after its reader stopped waiting; \
                     what did not arrive whole is not held",
                );
            }
        }
        None => error!(
            operation = "fetch_entry",
            "the rest of a parcel read panicked after its Entry was answered; \
             what did not arrive whole is not held",
        ),
    }
}
