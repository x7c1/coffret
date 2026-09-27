use serde::Serialize;

use crate::freeze::FreezeRun;
use crate::latest::Latest;
use crate::routes::RefusalDto;

use super::{named_folders, DisplacedFreezeDto, FindingDto, StepDto};

#[derive(Serialize)]
pub(super) struct FreezeDto {
    /// Which run of the freeze this is, counted from the start of this process.
    run: u64,
    /// The folder being packed; the Library root is the empty string, as it is
    /// in a listing.
    folder: String,
    /// `freezing`, `done` or `stopped`.
    status: &'static str,
    /// How many Packs the run built, and `0` until it is over.
    packs: usize,
    /// How many Entries those Packs hold, and `0` until it is over.
    entries: usize,
    /// What the run found and did not act on — a page inside a Pack it cannot
    /// replace, a mapped root the device could not vouch for. A page a Pack
    /// holds and that did not change is not among these: it is not eligible in
    /// the first place (spec: PK-1), and a second run over a book saying so of
    /// every page would be a wall of findings about nothing.
    findings: Vec<FindingDto>,
    /// How far into the run the flow says it has got, and `null` before it has
    /// said and once the run is over.
    ///
    /// The one thing a person dropping several hundred pages had no way to see:
    /// the counts above are outcomes and stay `0` until the batch commits, and
    /// this is the run moving while it builds it.
    step: Option<StepDto>,
    /// The books waiting their turn behind this one, oldest first.
    ///
    /// The names and not the length, because the length is the least of it: a
    /// person who dropped a second book wants to know that *their* book is
    /// queued, and a bare `1` says only that somebody's is. A freeze commits one
    /// batch (spec: PK-7), so a second one waits rather than superseding the
    /// first, and until it starts there is nothing else on the screen about it.
    waiting: Vec<String>,
    /// The books thrown away when the work ended without an answer, and that
    /// nobody has asked for since.
    discarded: Vec<String>,
    /// The runs that stopped and that a later one took the record from, oldest
    /// first.
    ///
    /// A freeze Storage stopped leaves a folder of pages on the disk and out of
    /// the Library, in a folder the browser made and the Library has never heard
    /// of — so this run is the only thing on the wire naming that place. The next
    /// book off the queue takes the record from it, and a second book queued
    /// behind the first is all it takes: without these the folder would be in
    /// none of the lists a page reads back, and a reload would draw no row for
    /// it, offer no way to walk into it and make no second attempt at it.
    ///
    /// The field above this one is not what these are: those books were thrown
    /// away before anything started on them, and these ran and got part way.
    ///
    /// Each carries what its own run came to and nothing of the flow: the three
    /// lists above belong to the queue rather than to any run, so they are the
    /// run on record's to report, and a displaced run does not carry them.
    displaced: Vec<DisplacedFreezeDto>,
    /// What stopped the freeze, and `null` where nothing did — which is exactly
    /// where `status` is not `stopped`.
    stopped: Option<RefusalDto>,
}

impl FreezeDto {
    /// The whole of what the flow has to say: the run on record, with the queue's
    /// two lists and the runs it took the record from beside it.
    pub(super) fn of(latest: &Latest<FreezeRun>) -> Self {
        let run = &latest.on_record;
        Self {
            run: run.run,
            folder: run.folder.as_str().to_owned(),
            status: run.status.as_str(),
            packs: run.packs,
            entries: run.entries,
            findings: run.findings.iter().map(FindingDto::of).collect(),
            step: run.step.as_ref().map(StepDto::of),
            waiting: named_folders(&latest.waiting),
            discarded: named_folders(&latest.discarded),
            displaced: latest
                .displaced
                .iter()
                .map(DisplacedFreezeDto::of)
                .collect(),
            stopped: run.status.stopped().map(RefusalDto::of),
        }
    }
}
