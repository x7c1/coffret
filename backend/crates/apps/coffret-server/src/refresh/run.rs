use std::time::Instant;

use coffret_device::CatchUpOutcome;
use tracing::info;

use crate::activity_log::{ActivityLog, CaughtUp, Ended};
use crate::api_error::ApiError;
use crate::refresh::Catalog;
use crate::reported::Reported;
use crate::state::ServerState;

/// One catch-up, with nobody else replaying at the same time.
///
/// Every caller reaches the Library through here, so the turn is taken, the line
/// is written and how the catalog stands is recorded once however the run was
/// asked for; `operation` is which of them asked.
///
/// The standing is written here rather than by the callers because this is the
/// only place that sees both ends of a replay — including the end that is not
/// an end at all, which is [`Replaying`]. What it is *not* is a second record of
/// the failure: the refusal goes back to whoever asked and is logged on its way
/// out, which is why it is kept through
/// [`Reported::of`](crate::reported::Reported::of) rather than recorded again.
///
/// Where `logged` is [`Logging::InTheLog`] — a catch-up the explorer asked for —
/// how it ended is also put in the server's log of finished work, before the
/// catalog says it ended, so no page reads it as over that the log does not
/// yet hold (spec: LA-12). One a locked Library refuses is refused before it
/// begins, and is not logged.
pub(super) async fn catch_up(
    state: &ServerState,
    operation: &'static str,
    logged: Logging,
) -> Result<CatchUpOutcome, ApiError> {
    let started = Instant::now();
    // Before the turn is taken, so that a locked server refuses at once rather
    // than queueing behind whoever is replaying (spec: DK-2), and held for the
    // whole replay: a catch-up that began unlocked finishes.
    let library = state.unlocked()?;
    let _turn = state.refreshes.turn().await;

    // After the turn, so that a caller queued behind a replay is not shown as
    // the replay: what this says is that the catalog is being caught up, and
    // between the two it is exactly as current as the last one left it.
    let log = match logged {
        Logging::InTheLog => Some(&*state.activity),
        Logging::Not => None,
    };
    let mut replaying = Replaying::started(&state.catalog, log);
    let outcome = match library.catch_up().await {
        Ok(outcome) => outcome,
        Err(error) => {
            let refusal = ApiError::from(error);
            replaying.stopped(Reported::of(&refusal));
            return Err(refusal);
        }
    };
    replaying.landed(CaughtUp::of(&outcome));
    // What it came to, and how long the caller waited — the wait for whoever was
    // replaying first included, since that is the time the request took. The
    // generations the catalog moved between are in the flow's own line and not
    // repeated here; nothing that arrived is named at all, an Entry Path being
    // the user's own name for their file (spec: EL-1).
    info!(
        operation,
        advanced = outcome.advanced(),
        gained = outcome.gained(),
        entries = outcome.entries_after,
        elapsed_ms = started.elapsed().as_millis(),
        "the catalog was caught up with the Library",
    );
    Ok(outcome)
}

/// Whether a catch-up is put in the server's log of finished work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Logging {
    /// Logged, as one the explorer asked for is (spec: LA-12).
    InTheLog,
    /// Not logged: one the server made on its own.
    Not,
}

/// A catch-up in flight, and what the catalog is left saying however it ends.
///
/// A guard rather than three lines in [`catch_up`], because one of the ways a
/// replay ends reaches no line at all: the future is dropped where it stands.
/// That happens twice over — the startup catch-up is on a deadline of its own,
/// and a request whose caller goes away is abandoned by the server — and both
/// would otherwise leave the catalog saying it is being caught up by a run that
/// no longer exists. A browser reading that is told to wait for nothing, and
/// goes on asking for as long as the tab is open.
///
/// So what a dropped replay leaves is what a refused one leaves: a catalog that
/// is behind, with the one thing there is to say about a call that never came
/// back.
///
/// Where it is one the log keeps, the log is told first, whichever way it
/// ended — a dropped one included, as what stopped it.
struct Replaying<'a> {
    catalog: &'a Catalog,
    /// The log it is put in as it ends, if it is one the log keeps.
    log: Option<&'a ActivityLog>,
    /// Whether the run said how it ended.
    ended: bool,
}

impl<'a> Replaying<'a> {
    /// Marks the catalog as being caught up, from now until this is dropped.
    fn started(catalog: &'a Catalog, log: Option<&'a ActivityLog>) -> Self {
        catalog.catching_up();
        Self {
            catalog,
            log,
            ended: false,
        }
    }

    /// The replay finished, so the catalog stands at the Library's head, and
    /// `caught_up` is what it came to.
    fn landed(&mut self, caught_up: CaughtUp) {
        self.log(Ok(caught_up));
        self.catalog.caught_up();
        self.ended = true;
    }

    /// The replay was refused, and `trouble` says what refused it.
    fn stopped(&mut self, trouble: Reported) {
        self.log(Err(trouble.clone()));
        self.catalog.behind(trouble);
        self.ended = true;
    }

    /// Puts how it ended in the log, if it is one the log keeps.
    fn log(&self, ended: Result<CaughtUp, Reported>) {
        if let Some(log) = self.log {
            log.record(Ended::CatchUp(ended));
        }
    }
}

impl Drop for Replaying<'_> {
    fn drop(&mut self) {
        if !self.ended {
            self.log(Err(Reported::gave_up()));
            self.catalog.behind(Reported::gave_up());
        }
    }
}
