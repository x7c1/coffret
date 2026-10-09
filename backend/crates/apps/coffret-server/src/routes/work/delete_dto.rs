use serde::Serialize;

use crate::delete::DeleteReport;
use crate::routes::{RefusalDto, RefusedPackDto};

use super::{FindingDto, StepDto};

#[derive(Serialize)]
pub(super) struct DeleteDto {
    /// Which run of the deletion this is, counted from the start of this
    /// process.
    run: u64,
    /// The folder it deletes, and `null` where it names files only.
    folder: Option<String>,
    /// The files it names, in Entry Path order.
    paths: Vec<String>,
    /// `deleting`, `done` or `stopped`.
    status: &'static str,
    /// How many files left the Library, and `0` until it is over.
    entries: usize,
    /// How many bytes those files came to, and `0` until it is over.
    bytes: u64,
    /// How many Containers were removed outright.
    removed: usize,
    /// How many Packs were rebuilt around the files they keep.
    rebuilt: usize,
    /// How many bytes the rebuilds read from Storage.
    rebuild_read: u64,
    /// How many bytes the rebuilt Packs weigh on Storage.
    rebuild_written: u64,
    /// The Packs it was refused for, with the named files each kept.
    refused: Vec<RefusedPackDto>,
    /// Named files the Library held nothing at.
    missing: Vec<String>,
    /// What its commit left for later and the Keyring repairs it performed.
    findings: Vec<FindingDto>,
    /// How far into the run the flow says it has got, and `null` before it has
    /// said and once the run is over.
    step: Option<StepDto>,
    /// How many deletions are waiting their turn behind this one.
    waiting: usize,
    /// What stopped the deletion, and `null` where nothing did — which is
    /// exactly where `status` is not `stopped`.
    stopped: Option<RefusalDto>,
}

impl DeleteDto {
    pub(super) fn of(report: &DeleteReport) -> Self {
        let run = &report.on_record;
        Self {
            run: run.run,
            folder: run.target.folder_named(),
            paths: run.target.paths_named(),
            status: run.status.as_str(),
            entries: run.entries,
            bytes: run.bytes,
            removed: run.removed,
            rebuilt: run.rebuilt,
            rebuild_read: run.rebuild_read,
            rebuild_written: run.rebuild_written,
            refused: run.refused.iter().map(RefusedPackDto::of).collect(),
            missing: run
                .missing
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            findings: run.findings.iter().map(FindingDto::of).collect(),
            step: run.step.as_ref().map(StepDto::of),
            waiting: report.waiting,
            stopped: run.status.stopped().map(RefusalDto::of),
        }
    }
}
