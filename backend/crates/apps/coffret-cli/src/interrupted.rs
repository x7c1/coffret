//! A run the person stopped with Ctrl-C.

use std::error;
use std::fmt;
use std::future::Future;
use std::thread;

use crate::answer::Ran;
use crate::report::Report;

/// The run was stopped by the person at the terminal before it answered.
///
/// What it was doing was dropped where it stood, and nothing it did is undone:
/// a commit is all or nothing (spec: CP-1), and what an interrupted run leaves
/// on this device — pending rows, a spool, an object on Storage no head names —
/// is what the next sync settles (spec: OC-2, OC-3, OC-7). The pending rows and
/// the start-up claim it held are released as they are dropped, so that next
/// run is not refused for them.
///
/// The process exits with [`Report::INTERRUPTED`], 130, however it then ends.
/// Work the run handed to a blocking thread — a file being read or written —
/// is not dropped with it, and the process waits for it before it exits; a
/// second Ctrl-C does not wait, and exits at once with the same status. What
/// that leaves is no different: a commit is all or nothing either way.
#[derive(Debug)]
pub struct Interrupted;

impl fmt::Display for Interrupted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "the run was interrupted — the next `coffret sync` settles whatever it left \
             unfinished; a second Ctrl-C stops it at once",
        )
    }
}

impl error::Error for Interrupted {}

/// The run, or the person pressing Ctrl-C, whichever comes first.
///
/// Where the signal wins, the run's future is dropped where it stands — the
/// whole of the cancellation, and [`Interrupted`] says what that leaves — and
/// the run fails through the same path as any other.
///
/// The run is polled first, every time, so the signal is not listened for
/// until the run has first had to wait: a Passphrase prompt reads the terminal
/// inside that first poll, and Ctrl-C at it stops the process as it always
/// did, rather than being taken by a listener the blocked prompt never lets
/// answer.
pub(crate) async fn raced(
    run: impl Future<Output = anyhow::Result<Ran>>,
    interrupt: impl Future<Output = ()>,
) -> anyhow::Result<Ran> {
    tokio::select! {
        biased;
        ran = run => ran,
        () = interrupt => Err(Interrupted.into()),
    }
}

/// Resolves when the person presses Ctrl-C, and never where it could not be
/// listened for: a run that cannot be interrupted cleanly is still a run, and
/// Ctrl-C then stops the process as it would have without this.
///
/// Once it has resolved, a second Ctrl-C exits the process at once with
/// [`Report::INTERRUPTED`]. The handler listening for the first one stays
/// installed, so without this the second would be taken and ignored while the
/// process waits for the work the run left on blocking threads.
pub(crate) async fn ctrl_c() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(
            kind = ?error.kind(),
            "Ctrl-C could not be listened for; the run goes on without it",
        );
        std::future::pending::<()>().await;
    }
    tracing::info!("the run was interrupted by Ctrl-C");
    stop_on_second(second_ctrl_c, |status| {
        std::process::exit(i32::from(status))
    });
}

/// Waits, on a thread of its own, for `second` to say the person pressed
/// Ctrl-C again, and then calls `stop` with [`Report::INTERRUPTED`].
///
/// A thread of its own rather than a task, because the moment it is needed is
/// the one the runtime has stopped running tasks: the run has been dropped,
/// `main` has returned, and the runtime is waiting for its blocking threads to
/// finish. Where no thread can be started, Ctrl-C again does what it did
/// before — nothing — and the process exits once that work has finished.
fn stop_on_second(
    second: impl FnOnce() -> bool + Send + 'static,
    stop: impl FnOnce(u8) + Send + 'static,
) -> Option<thread::JoinHandle<()>> {
    let watching = thread::Builder::new()
        .name("second-ctrl-c".to_owned())
        .spawn(move || {
            if second() {
                stop(Report::INTERRUPTED);
            }
        });
    match watching {
        Ok(watching) => Some(watching),
        Err(error) => {
            second_not_listened(error.kind());
            None
        }
    }
}

/// Blocks until the person presses Ctrl-C, on a runtime of the calling thread's
/// own, and says whether they did: `false` where it could not be listened for.
fn second_ctrl_c() -> bool {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            second_not_listened(error.kind());
            return false;
        }
    };
    match runtime.block_on(tokio::signal::ctrl_c()) {
        Ok(()) => {
            tracing::info!("a second Ctrl-C stopped the run without waiting for its work");
            true
        }
        Err(error) => {
            second_not_listened(error.kind());
            false
        }
    }
}

/// Logs that a second Ctrl-C cannot be listened for, and the kind of failure
/// that kept it from being.
fn second_not_listened(kind: std::io::ErrorKind) {
    tracing::warn!(
        ?kind,
        "a second Ctrl-C could not be listened for; the run exits once its work is done",
    );
}

#[cfg(test)]
mod tests {
    use std::future;

    use coffret_device::Findings;

    use super::*;
    use crate::answer::{Document, Failure};
    use crate::report;

    // Ctrl-C wins over a run still under way: the run's future is dropped, and
    // what the person reads is the one line saying it was interrupted, that
    // the next sync settles what it left, and that a second Ctrl-C stops at
    // once — through the ordinary failure, which exits with 130 rather than
    // the 1 of a run that failed on its own.
    #[tokio::test]
    async fn a_run_interrupted_by_ctrl_c_fails_saying_so() {
        let error = raced(future::pending(), future::ready(()))
            .await
            .err()
            .expect("the signal wins over a run that has not answered");

        assert!(error.is::<Interrupted>(), "{error:#}");
        let said = report::failed(&error).said;
        assert_eq!(
            said,
            [
                "the run was interrupted — the next `coffret sync` settles whatever it left \
                 unfinished; a second Ctrl-C stops it at once"
            ],
        );
        let answered: serde_json::Value = serde_json::from_str(
            &Document::failed(
                Some("sync"),
                None,
                Failure::of(&error),
                &Findings::default(),
            )
            .rendered(),
        )
        .expect("the answer is JSON");
        assert_eq!(answered["error"]["kind"], "interrupted", "{answered}");
        assert_eq!(answered["exit_status"], 130, "{answered}");
        assert_eq!(Failure::of(&error).exit_status(), 130);
    }

    // A run that failed on its own exits 1, as it always did: 130 is a run
    // somebody stopped, and nothing else.
    #[test]
    fn a_run_that_failed_on_its_own_exits_one() {
        let error = anyhow::anyhow!("the run's own failure");
        assert_eq!(Failure::of(&error).exit_status(), 1);
    }

    // A second Ctrl-C, once the first has interrupted the run, stops the
    // process at once with 130 — from a thread of its own, so nothing the
    // runtime is waiting for keeps it from being heard.
    #[test]
    fn a_second_ctrl_c_stops_at_once_with_130() {
        let (stopped, said) = std::sync::mpsc::channel();
        let watching = stop_on_second(
            || true,
            move |status| stopped.send(status).expect("the case is listening"),
        )
        .expect("a thread can be started");
        assert_eq!(
            said.recv_timeout(std::time::Duration::from_secs(5)),
            Ok(130)
        );
        watching.join().expect("the watching thread ends");
    }

    // Where a second Ctrl-C could not be listened for, nothing stops the
    // process: it exits once its work is done, with the status the run failed
    // with.
    #[test]
    fn a_second_ctrl_c_that_cannot_be_heard_stops_nothing() {
        let (stopped, said) = std::sync::mpsc::channel();
        let watching = stop_on_second(
            || false,
            move |status| stopped.send(status).expect("the case is listening"),
        )
        .expect("a thread can be started");
        watching.join().expect("the watching thread ends");
        assert!(said.try_recv().is_err(), "nothing was stopped");
    }

    // A run that answers before any signal answers as it would have.
    #[tokio::test]
    async fn a_run_that_answers_first_is_not_interrupted() {
        let error = raced(
            future::ready(Err(anyhow::anyhow!("the run's own failure"))),
            future::pending(),
        )
        .await
        .err()
        .expect("the run failed on its own");
        assert!(!error.is::<Interrupted>(), "{error:#}");
    }

    // The run is polled before the signal is listened for, so a run that
    // answers in its first poll — a Passphrase prompt included — answers even
    // where Ctrl-C would be ready at once.
    #[tokio::test]
    async fn the_run_is_polled_before_the_signal() {
        let error = raced(
            future::ready(Err(anyhow::anyhow!("the run's own failure"))),
            future::ready(()),
        )
        .await
        .err()
        .expect("the run failed on its own");
        assert!(!error.is::<Interrupted>(), "{error:#}");
    }
}
