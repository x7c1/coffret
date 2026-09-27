use serde::Serialize;

use crate::displaced::Displaced;
use crate::freeze::FreezeRun;
use crate::routes::RefusalDto;

use super::{FindingDto, StepDto, STOPPED};

/// A freeze that stopped and that a later one took the record from.
///
/// Its own shape for the reason [`DisplacedFillDto`](super::DisplacedFillDto) is: it stopped, so its
/// status is always `stopped` and it always says what stopped it, and the
/// queue's lists are not its to carry.
#[derive(Serialize)]
pub(super) struct DisplacedFreezeDto {
    /// Which run of the freeze this was.
    run: u64,
    /// The folder it was packing.
    folder: String,
    /// Always `stopped`: a run is only ever displaced from there.
    status: &'static str,
    /// How many Packs it built, which is `0` for a run that stopped before its
    /// batch committed.
    packs: usize,
    /// How many Entries those Packs hold.
    entries: usize,
    /// What it found and did not act on.
    findings: Vec<FindingDto>,
    /// How far it had got, which a stopped run no longer says.
    step: Option<StepDto>,
    /// What stopped it.
    stopped: RefusalDto,
}

impl DisplacedFreezeDto {
    pub(super) fn of(displaced: &Displaced<FreezeRun>) -> Self {
        let run = &displaced.run;
        Self {
            run: run.run,
            folder: run.folder.as_str().to_owned(),
            status: STOPPED,
            packs: run.packs,
            entries: run.entries,
            findings: run.findings.iter().map(FindingDto::of).collect(),
            step: run.step.as_ref().map(StepDto::of),
            stopped: RefusalDto::of(&displaced.stopped),
        }
    }
}
