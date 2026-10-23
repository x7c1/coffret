//! A run the person stopped with Ctrl-C.

use std::error;
use std::fmt;
use std::future::Future;

use crate::answer::Ran;

/// The run was stopped by the person at the terminal before it answered.
///
/// What it was doing was dropped where it stood, and nothing it did is undone:
/// a commit is all or nothing (spec: CP-1), and what an interrupted run leaves
/// on this device — pending rows, a spool, an object on Storage no head names —
/// is what the next sync settles (spec: OC-2, OC-3, OC-7). The pending rows and
/// the start-up claim it held are released as they are dropped, so that next
/// run is not refused for them.
#[derive(Debug)]
pub struct Interrupted;

impl fmt::Display for Interrupted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "the run was interrupted — the next `coffret sync` settles whatever it left unfinished",
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
pub(crate) async fn ctrl_c() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(
            kind = ?error.kind(),
            "Ctrl-C could not be listened for; the run goes on without it",
        );
        std::future::pending::<()>().await;
    }
    tracing::info!("the run was interrupted by Ctrl-C");
}

#[cfg(test)]
mod tests {
    use std::future;

    use coffret_device::Findings;

    use super::*;
    use crate::answer::{Document, Failure};
    use crate::report;

    // Ctrl-C wins over a run still under way: the run's future is dropped, and
    // what the person reads is the one line saying it was interrupted and that
    // the next sync settles what it left — through the ordinary failure, so
    // the exit status is the failure's.
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
              unfinished"
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
        assert_ne!(answered["exit_status"], 0, "{answered}");
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
