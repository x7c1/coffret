use std::sync::Arc;

use coffret_model::Generation;
use tracing::info;

use crate::commit::{catch_up, CommitResult};
use crate::index::Index;
use crate::sync::{complete_after_catch_up, PendingWork};

use super::catch_up_outcome::CatchUpOutcome;
use super::catch_up_request::CatchUpRequest;

/// Brings this device's catalog to the Library's current head (spec: CK-9).
///
/// The whole of it is [`commit::catch_up`](crate::commit) with the two counts a
/// caller needs to say what happened taken either side of it: the head the Index
/// stood at, and how many current Entries it held. Nothing else runs — no scan of
/// the mapped folders, no spool written, no Container read, no file placed —
/// because a Journal record carries what the Containers it adds hold
/// (spec: CP-11), which is exactly what makes catching a catalog up cost the
/// control objects alone.
///
/// Save one thing, which is this device's own bookkeeping rather than anything
/// of the Library's: a batch of this device's whose record landed and whose
/// refresh did not is committed, and a catalog the replay has just brought to
/// the head says so. Its pending rows are completed then and there, as a sync's
/// settlement would complete them, unless another run owns them (spec: OC-2,
/// OC-7) — so the files it carried in read as this device's from this catch-up
/// on, rather than from the next sync's.
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
        spool,
        now,
        policy,
        memory,
    } = request;

    let from = head_of(index).await?;
    let entries_before = index.entries_under(None).await?.len();

    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    let to = head_of(index).await?;
    if let Some(memory) = memory {
        // Kept with the head it brought the catalog to, which is what makes it
        // the listing of this catalog rather than of whatever is newer. A
        // cache: the catalog has advanced whether or not it is kept.
        memory
            .remember_listing(index, Arc::new(caught.listing), to)
            .await;
        // And this is where a repair made since is learned of, under a
        // commitment that need not have moved: a finding that the set is short,
        // kept from an earlier read, is not carried past it (spec: KL-13, KL-15).
        memory.forget_short_key_table();
    }
    complete_after_catch_up(index, spool, now, PendingWork::Unowned).await;

    let outcome = CatchUpOutcome {
        from,
        to,
        entries_before,
        entries_after: index.entries_under(None).await?.len(),
    };
    finished(&outcome);
    Ok(outcome)
}

/// The head the catalog stands at, or `None` where it stands at no committed
/// state.
async fn head_of(index: &dyn Index) -> CommitResult<Option<Generation>> {
    Ok(index.checkpoint().await?.map(|at| at.head_generation()))
}

/// Records what the run came to, in generations and counts alone.
///
/// No Entry Path reaches the diagnostic event: what a catch-up learned is the
/// user's own names for their own files (spec: EL-1), and how many there now
/// are is enough to read a run's account of itself.
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
