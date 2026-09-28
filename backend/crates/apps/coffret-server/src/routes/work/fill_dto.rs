use serde::Serialize;

use crate::fill::FillRun;
use crate::latest::Latest;
use crate::routes::RefusalDto;

use super::{named_folders, DeclinedDto, DisplacedFillDto, FindingDto};

#[derive(Serialize)]
pub(super) struct FillDto {
    /// Which run of the fill this is, counted from the start of this process.
    ///
    /// What tells one run's account of itself from the next's, so that a line
    /// somebody has read and put away does not take the next run's with it.
    run: u64,
    /// The folder being brought over; the Library root is the empty string, as
    /// it is in a listing.
    folder: String,
    /// `filling`, `done`, `stopped` or `superseded`.
    status: &'static str,
    /// How many of the folder's files the fill set out to bring over, and `0`
    /// until it has read the folder's listing.
    total: usize,
    /// How many of them are on this device now.
    done: usize,
    /// The Entries it did not bring over, each with what the file route would
    /// have said about it — so a row can be marked without anyone clicking it.
    declined: Vec<DeclinedDto>,
    /// What the run found and did not act on: what its reads found of the
    /// committed Keyring, where one had to step over a position of the set,
    /// said once however many Entries met it.
    ///
    /// The findings a sync and a freeze carry, in their shape and with their
    /// sentences, and like theirs none of it stops the run: the files still
    /// open. It is here because the person it matters most for is one who only
    /// reads — opens files in the explorer and never syncs or freezes there —
    /// and a fill is the one run their reading makes (spec: KL-15).
    findings: Vec<FindingDto>,
    /// The folders somebody asked for by name that are waiting their turn behind
    /// this one, oldest first.
    ///
    /// The names and not the length, for the reason the freeze's queue carries
    /// names: a person who pressed a button wants to know that *their* folder is
    /// coming. A folder asked for by name waits its turn rather than superseding
    /// the fill in progress, and between the press and the run this is the only
    /// thing on the wire about it — the line names the folder being brought over,
    /// and the button that named this one is gone the moment the queue takes it.
    waiting: Vec<String>,
    /// The folders that were queued behind a fill and thrown away when the work
    /// ended without an answer, and that nobody has taken up since.
    ///
    /// Not this fill's own doing and not gone when it is: it is the queue that
    /// lost them, and what answers one is somebody asking for that folder again.
    /// Without it the line and the retry both name the folder that died, and the
    /// folder somebody clicked into afterwards is never mentioned at all.
    discarded: Vec<String>,
    /// The runs that stopped and that a later one took the record from, oldest
    /// first — the newest eight at most, the oldest forgotten past that (see
    /// the fill's `Progress`).
    ///
    /// A fill Storage stopped is a folder somebody asked for and did not get,
    /// and the field above this one is not what it is: those folders were thrown
    /// away before anything started on them, and these ran and got part way. The
    /// next folder taken off the queue takes the record from a stopped run
    /// within a tick — a person clicking into another folder while Storage is
    /// down is enough — so without these the line, the Entries it declined and
    /// the offer of a second attempt would all go unread.
    ///
    /// Each carries what its own run came to and nothing of the flow: the three
    /// lists above belong to the queue rather than to any run, so they are the
    /// run on record's to report, and a displaced run does not carry them.
    displaced: Vec<DisplacedFillDto>,
    /// What stopped the fill, and `null` where nothing did — which is exactly
    /// where `status` is not `stopped`.
    stopped: Option<RefusalDto>,
}

impl FillDto {
    /// The whole of what the flow has to say: the run on record, with the queue's
    /// two lists and the runs it took the record from beside it.
    pub(super) fn of(latest: &Latest<FillRun>) -> Self {
        let run = &latest.on_record;
        Self {
            run: run.run,
            folder: run.folder.as_str().to_owned(),
            status: run.status.as_str(),
            total: run.total,
            done: run.done,
            declined: run.declined.iter().map(DeclinedDto::of).collect(),
            findings: run.findings().iter().map(FindingDto::of).collect(),
            waiting: named_folders(&latest.waiting),
            discarded: named_folders(&latest.discarded),
            displaced: latest.displaced.iter().map(DisplacedFillDto::of).collect(),
            stopped: run.status.stopped().map(RefusalDto::of),
        }
    }
}
