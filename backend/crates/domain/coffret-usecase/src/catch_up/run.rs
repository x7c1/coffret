use coffret_model::Generation;
use tracing::info;

use crate::commit::{catch_up, CommitResult};
use crate::index::Index;

use super::catch_up_outcome::CatchUpOutcome;
use super::catch_up_request::CatchUpRequest;

/// Brings this device's catalog to the Library's current head (spec: CK-9).
///
/// The whole of it is [`commit::catch_up`](crate::commit) with the two counts a
/// caller needs to say what happened taken either side of it: the head the Index
/// stood at, and how many current Entries it held. Nothing else runs — no scan of
/// the mapped folders, no spool, no Container read, no file placed — because a
/// Journal record carries what the Containers it adds hold (spec: CP-11), which
/// is exactly what makes catching a catalog up cost the control objects alone.
///
/// The counts are read from the Index rather than derived from what was
/// replayed, and that is deliberate: a record's additions and removals are not
/// the same as the Entries a catalog gained, since an addition may replace a
/// current Entry at the same path (spec: EP-6). What it costs is walking the
/// current Entries twice, which is local and is the same walk a Library-wide
/// listing makes — the port answers with the Entries and not with a count, and a
/// count of its own on [`Index`] would be a twentieth operation for one status
/// line.
///
/// A run that reaches the head it was already at is a success with nothing to
/// report, which is the ordinary answer: `advanced` is false and the caller says
/// so. Failure is the commit flow's own vocabulary and is left in it — a
/// catch-up fails at reading Storage and at the control state it finds there, and
/// nothing else, so a second error type here would be a second spelling of one
/// verdict.
pub async fn catch_up_catalog(request: CatchUpRequest<'_>) -> CommitResult<CatchUpOutcome> {
    let CatchUpRequest {
        store,
        index,
        keys,
        policy,
    } = request;

    let from = head_of(index).await?;
    let entries_before = index.entries_under(None).await?.len();

    catch_up(store, index, keys.control(), &policy.retry).await?;

    let outcome = CatchUpOutcome {
        from,
        to: head_of(index).await?,
        entries_before,
        entries_after: index.entries_under(None).await?.len(),
    };
    finished(&outcome);
    Ok(outcome)
}

/// The head the catalog stands at, or `None` where it stands at no committed
/// state.
async fn head_of(index: &dyn Index) -> CommitResult<Option<Generation>> {
    Ok(index.checkpoint().await?.map(|at| at.head_generation))
}

/// Records what the run came to, in generations and counts alone.
///
/// No Entry Path reaches the line: what a catch-up learned is the user's own
/// names for their own files (spec: EP-1), and how many there now are is enough
/// to read a run's account of itself.
fn finished(outcome: &CatchUpOutcome) {
    info!(
        operation = "catch_up",
        from = outcome.from.map(Generation::get),
        to = outcome.to.map(Generation::get),
        gained = outcome.gained(),
        entries = outcome.entries_after,
        "the catalog was caught up with the Library's head",
    );
}
