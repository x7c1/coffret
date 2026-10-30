use serde::Serialize;

use crate::moves::MoveRun;
use crate::routes::RefusalDto;

use super::{FindingDto, PairDto};

/// What a move did, as the `POST` answers it and the log keeps it.
#[derive(Serialize)]
pub(crate) struct MoveDto {
    /// The pairs it was asked to move.
    pairs: Vec<PairDto>,
    /// `done` or `stopped`.
    status: &'static str,
    /// How many Entries the record renamed.
    entries: usize,
    /// How many of this device's files followed their Entries.
    relocated: usize,
    /// How many of them could not follow and left this device.
    left: usize,
    /// What kept this device's files from following the renamed Entries, on a
    /// move that is `done` and whose relocations failed after its commit, and
    /// `null` everywhere else: the files stay where they were, and the next
    /// sync or fetch relocates them.
    unrelocated: Option<RefusalDto>,
    /// What became of this device's files, what the commit left for later
    /// and the Keyring repairs it performed.
    findings: Vec<FindingDto>,
    /// What stopped the move, and `null` where nothing did — which is exactly
    /// where `status` is not `stopped`.
    stopped: Option<RefusalDto>,
}

impl MoveDto {
    pub(crate) fn of(run: &MoveRun) -> Self {
        Self {
            pairs: run.pairs.iter().map(PairDto::of).collect(),
            status: run.status.as_str(),
            entries: run.entries,
            relocated: run.relocated,
            left: run.left,
            unrelocated: run.unrelocated.as_ref().map(RefusalDto::of),
            findings: run.findings.iter().map(FindingDto::of).collect(),
            stopped: run.status.stopped().map(RefusalDto::of),
        }
    }
}
