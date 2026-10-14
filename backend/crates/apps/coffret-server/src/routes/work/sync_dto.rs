use serde::Serialize;

use crate::pending_rows::TurnTaker;
use crate::routes::RefusalDto;
use crate::sync::{SyncRun, SyncStatus};

use super::{waits, FindingDto, StepDto};

#[derive(Serialize)]
pub(super) struct SyncDto {
    /// Which run of the sync this is, counted from the start of this process.
    run: u64,
    /// `syncing`, `done` or `stopped`.
    status: &'static str,
    /// How many files the run carried into the Library, and `0` until it is
    /// over.
    added: usize,
    /// What the run found and did not act on — a file inside a Pack it cannot
    /// replace, a file this device no longer has, a mapped root the device
    /// could not vouch for — and the Keyring repairs its commit performed, on
    /// a run that stopped as well as one that finished.
    findings: Vec<FindingDto>,
    /// How far into the walk the flow says it has got, and `null` before it has
    /// said and once the run is over.
    step: Option<StepDto>,
    /// Whether the run is waiting for its turn at the pending rows: true from
    /// the moment it is on record until it holds the turn, and false once it
    /// does and once it is over. A run with no step that is not waiting has
    /// taken the turn and not said how far it has got yet.
    waits: bool,
    /// What stopped the sync, and `null` where nothing did — which is exactly
    /// where `status` is not `stopped`.
    stopped: Option<RefusalDto>,
}

impl SyncDto {
    pub(super) fn of(run: &SyncRun, turn: Option<TurnTaker>) -> Self {
        Self {
            run: run.run,
            status: run.status.as_str(),
            added: run.added,
            findings: run.findings.iter().map(FindingDto::of).collect(),
            step: run.step.as_ref().map(StepDto::of),
            waits: waits(
                matches!(run.status, SyncStatus::Syncing),
                TurnTaker::Sync,
                turn,
            ),
            stopped: run.status.stopped().map(RefusalDto::of),
        }
    }
}
