use serde::Serialize;

use crate::routes::RefusalDto;
use crate::sync::SyncRun;

use super::{FindingDto, StepDto};

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
    /// could not vouch for.
    findings: Vec<FindingDto>,
    /// How far into the walk the flow says it has got, and `null` before it has
    /// said and once the run is over.
    step: Option<StepDto>,
    /// What stopped the sync, and `null` where nothing did — which is exactly
    /// where `status` is not `stopped`.
    stopped: Option<RefusalDto>,
}

impl SyncDto {
    pub(super) fn of(run: &SyncRun) -> Self {
        Self {
            run: run.run,
            status: run.status.as_str(),
            added: run.added,
            findings: run.findings.iter().map(FindingDto::of).collect(),
            step: run.step.as_ref().map(StepDto::of),
            stopped: run.status.stopped().map(RefusalDto::of),
        }
    }
}
