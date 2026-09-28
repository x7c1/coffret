use serde::Serialize;

use crate::displaced::Displaced;
use crate::fill::FillRun;
use crate::routes::RefusalDto;

use super::{DeclinedDto, FindingDto, STOPPED};

/// A fill that stopped and that a later one took the record from.
///
/// Its own shape rather than a [`FillDto`](super::FillDto) with the queue's lists left empty,
/// because what it is is narrower than any fill: it stopped, so its status is
/// always `stopped` and it always says what stopped it. What it keeps is what
/// its line and its rows are drawn from — the counts, and the Entries it
/// declined, which stay marked after the next run has the record.
#[derive(Serialize)]
pub(super) struct DisplacedFillDto {
    /// Which run of the fill this was.
    run: u64,
    /// The folder it was bringing over.
    folder: String,
    /// Always `stopped`: a run is only ever displaced from there.
    status: &'static str,
    /// How many of the folder's files it set out to bring over.
    total: usize,
    /// How many of them it had brought over when it stopped.
    done: usize,
    /// The Entries it declined, each with its refusal — not the ones it stopped
    /// before reaching.
    declined: Vec<DeclinedDto>,
    /// What it found and did not act on before it stopped, as a fill on
    /// record carries it.
    findings: Vec<FindingDto>,
    /// What stopped it.
    stopped: RefusalDto,
}

impl DisplacedFillDto {
    pub(super) fn of(displaced: &Displaced<FillRun>) -> Self {
        let run = &displaced.run;
        Self {
            run: run.run,
            folder: run.folder.as_str().to_owned(),
            status: STOPPED,
            total: run.total,
            done: run.done,
            declined: run.declined.iter().map(DeclinedDto::of).collect(),
            findings: run.findings().iter().map(FindingDto::of).collect(),
            stopped: RefusalDto::of(&displaced.stopped),
        }
    }
}
