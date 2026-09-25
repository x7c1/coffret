use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use coffret_device::{Phase, Step};

use crate::fill::{Activity, Declined};
use crate::finding::Finding;
use crate::folder::Folder;
use crate::freeze::FreezeActivity;
use crate::latest::Latest;
use crate::refresh::Standing;
use crate::reported::Reported;
use crate::state::ServerState;
use crate::sync::SyncActivity;

/// What the server is doing on its own, which is three things — and the one
/// thing it may have done to itself.
///
/// A fill, a sync and a freeze. Everything else this server does it does because
/// a request asked it to, and a request is answered rather than reported on;
/// these three are the work nobody asked for — the rest of a folder being
/// brought over behind a reader, files somebody dropped being carried into the
/// Library, and a book somebody brought in being packed — so they are what there
/// is to tell a browser about.
///
/// Side by side and not one after another: they are separate work over one
/// Library, any of them can be running without the others, and a browser reads
/// each on its own.
///
/// The first two fields are not work. Whether this device still holds the
/// Library open is the one thing about itself a browser cannot find out by
/// waiting: the idle lock happens on this server's clock and tells nobody
/// (spec: DK-4), so a window left open over a page it decrypted would go on
/// showing that page until something it asked for happened to be refused. It
/// rides here because this is the question an open explorer is already asking,
/// and because asking it keeps nothing awake.
///
/// How the catalog stands rides here for the same reason and answers the same
/// kind of question. Every listing comes out of the catalog, and the catalog is
/// what this device has replayed (spec: CK-9) — so a device fresh from `join`
/// whose catch-up did not land serves an empty Library that is not empty, and no
/// request a screen makes for its own sake would ever say so. This is the one a
/// page asks as it comes up.
///
/// And the first field of all is not about the Library either: it is which
/// process answered. Everything below it that a browser remembers between
/// answers — which run's line somebody read and put away — is true of one
/// process only, because the run numbers start again at 1 with the next one.
#[derive(Serialize)]
pub struct ActivityDto {
    /// What the process that answered calls itself.
    ///
    /// The same string for every answer this process gives and a different one
    /// after it is started again, which is all a page needs from it: what it
    /// remembers about runs it has been shown is about this server's runs, and
    /// a name it has not seen before is the sign to forget it. A page cannot
    /// reach that conclusion from the run numbers — one lower than a run it put
    /// away is equally what an answer issued just before the dismissal looks
    /// like.
    ///
    /// It says nothing about this device or this Library: see
    /// [`ServerId`](crate::server_id::ServerId) for what it is composed of and
    /// why it is composed rather than taken from something that was already
    /// there.
    server: String,
    /// Which of the two states this device holds the Library in, in the words
    /// DK-1 uses: `locked` or `unlocked`.
    library: &'static str,
    /// How this device's catalog stands with the Library.
    catalog: CatalogDto,
    /// The latest fill, running or finished, and `null` where none has run.
    fill: Option<FillDto>,
    /// The latest sync, running or finished, and `null` where none has run.
    sync: Option<SyncDto>,
    /// The latest freeze, running or finished, and `null` where none has run.
    freeze: Option<FreezeDto>,
}

/// How far this device has got with the Library, and what stopped it where
/// something did.
///
/// Two fields and not one word, because a person is owed the reason as well as
/// the state: "this device has not caught up" is something to wait through, and
/// "Storage did not answer" is something to press a button about.
#[derive(Serialize)]
struct CatalogDto {
    /// `catching_up`, `caught_up` or `behind`.
    state: &'static str,
    /// What stopped the last catch-up, and `null` where nothing did.
    trouble: Option<RefusalDto>,
}

/// How far into a run the flow reports it has got.
///
/// The [`Step`](coffret_device::Step) the use case layer reports, in the shape a
/// browser reads it in. It is the same value the command line renders its
/// progress line from, and deliberately so: one run has one answer about where
/// it is, whichever shell is watching.
#[derive(Serialize)]
struct StepDto {
    /// Which phase of the flow it is in.
    phase: &'static str,
    /// How many units of that phase are finished.
    done: usize,
    /// How many there are in all, and `null` where the phase cannot say.
    ///
    /// Absent rather than zero, and the two must not be shown alike: a phase
    /// that cannot count its work — a catch-up learns what it has to replay by
    /// replaying it — is exactly the phase that goes quiet for minutes, and a
    /// `0` there would read as a phase with nothing in it.
    total: Option<usize>,
}

#[derive(Serialize)]
struct FillDto {
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
    /// The folders somebody asked for by name that are waiting their turn behind
    /// this one, oldest first.
    ///
    /// The names and not the length, for the reason the freeze's queue carries
    /// names: a person who pressed a button wants to know that *their* folder is
    /// coming. A folder asked for by name waits rather than displacing the fill
    /// in progress, and between the press and the run this is the only thing on
    /// the wire about it — the line names the folder being brought over, and the
    /// button that named this one is gone the moment the queue takes it.
    waiting: Vec<String>,
    /// The folders that were queued behind a fill and thrown away when the work
    /// ended without an answer, and that nobody has taken up since.
    ///
    /// Not this fill's own doing and not gone when it is: it is the queue that
    /// lost them, and what settles one is somebody asking for that folder again.
    /// Without it the line and the retry both name the folder that died, and the
    /// folder somebody clicked into afterwards is never mentioned at all.
    dropped: Vec<String>,
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
    /// run on record's to report and arrive empty here.
    displaced: Vec<FillDto>,
    /// What stopped the fill, and `null` where nothing did.
    stopped: Option<RefusalDto>,
}

#[derive(Serialize)]
struct SyncDto {
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
    /// What stopped the sync, and `null` where nothing did.
    stopped: Option<RefusalDto>,
}

#[derive(Serialize)]
struct FreezeDto {
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
    /// batch (spec: PK-7), so a second one waits rather than displacing the
    /// first, and until it starts there is nothing else on the screen about it.
    waiting: Vec<String>,
    /// The books thrown away when the work ended without an answer, and that
    /// nobody has asked for since.
    dropped: Vec<String>,
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
    /// run on record's to report and arrive empty here.
    displaced: Vec<FreezeDto>,
    /// What stopped the freeze, and `null` where nothing did.
    stopped: Option<RefusalDto>,
}

/// One thing a run that succeeded still has to say.
///
/// Unlike a declined Entry this carries no refusal kind, and deliberately:
/// nothing was refused. The run succeeded and left this alone, so what there is
/// to show is the sentence and the row it belongs to — `null` for the findings
/// that are about no single Entry — and which finding it is, named beside the
/// sentence in the `reason` and `surfaced` a declined Entry names the same
/// state by, so that a page branching on it never has to read it out of prose.
#[derive(Serialize)]
struct FindingDto {
    path: Option<String>,
    message: String,
    /// One of the names [`Finding::reason`] lists.
    reason: &'static str,
    /// The device layer's name for a finding about one Entry, and absent for
    /// one about a mapping or a Container — left out rather than `null`, as a
    /// refusal's is, so the two read alike.
    #[serde(skip_serializing_if = "Option::is_none")]
    surfaced: Option<&'static str>,
}

#[derive(Serialize)]
struct DeclinedDto {
    path: String,
    #[serde(flatten)]
    refusal: RefusalDto,
}

/// One refusal, in the shape every refusal on these routes takes.
///
/// The same four fields under the same four names, so a browser reads a
/// declined Entry with the code it already has for a refused request. Shared
/// with the upload's per-part list for that reason: a refusal a person meets by
/// dropping a file and one they meet by opening it are one vocabulary.
#[derive(Serialize)]
pub(super) struct RefusalDto {
    error: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    surfaced: Option<&'static str>,
}

impl ActivityDto {
    /// What the server is doing right now, as the browser is told it.
    ///
    /// Read off the state rather than assembled by each caller: the three routes
    /// that answer with an activity all answer with the whole of it, and a
    /// caller that assembled two thirds of it would be a browser told that
    /// whichever work it did not name had stopped.
    ///
    /// Each of the two flows that keep a queue is read in one go rather than a
    /// field at a time. What a run is, what is waiting behind it and what its
    /// queue lost move together — a worker that dies stops the run and throws
    /// the rest away in one stroke — so three reads could put half of that
    /// change on the wire: a run still filling beside the folders its ending
    /// threw away, or a run still `done` beside a queue that has already been
    /// taken up, which is a browser told there is nothing left to follow at the
    /// moment the next run starts.
    pub fn of(state: &ServerState) -> Self {
        Self {
            server: state.server().to_owned(),
            library: if state.holds_library() {
                "unlocked"
            } else {
                "locked"
            },
            catalog: CatalogDto::of(&state.catalog.standing()),
            fill: state.fills.reported().as_ref().map(FillDto::of),
            sync: state.syncs.activity().as_ref().map(SyncDto::of),
            freeze: state.freezes.reported().as_ref().map(FreezeDto::of),
        }
    }
}

impl CatalogDto {
    fn of(standing: &Standing) -> Self {
        match standing {
            Standing::CatchingUp => Self {
                state: "catching_up",
                trouble: None,
            },
            Standing::CaughtUp => Self {
                state: "caught_up",
                trouble: None,
            },
            Standing::Behind(trouble) => Self {
                state: "behind",
                trouble: Some(RefusalDto::of(trouble)),
            },
        }
    }
}

impl StepDto {
    fn of(step: &Step) -> Self {
        Self {
            phase: named(step.phase),
            done: step.done,
            total: step.total,
        }
    }
}

/// The word one phase travels under.
///
/// Named here rather than on [`Phase`](coffret_device::Phase) because it is this
/// route's vocabulary: the use case layer says what a run is doing, and what a
/// browser calls it is the browser's business. Matched exhaustively, so a phase
/// added to the flow stops this compiling until somebody says what a person
/// reading a status bar should be told it is.
fn named(phase: Phase) -> &'static str {
    match phase {
        Phase::CatchingUp => "catching_up",
        Phase::Reconciling => "reconciling",
        Phase::Scanning => "scanning",
        Phase::Packing => "packing",
        Phase::Uploading => "uploading",
        Phase::Fetching => "fetching",
    }
}

impl SyncDto {
    fn of(activity: &SyncActivity) -> Self {
        Self {
            run: activity.run,
            status: activity.status.as_str(),
            added: activity.added,
            findings: activity.findings.iter().map(FindingDto::of).collect(),
            step: activity.step.as_ref().map(StepDto::of),
            stopped: activity.stopped.as_ref().map(RefusalDto::of),
        }
    }
}

impl FreezeDto {
    /// The whole of what the flow has to say: the run on record, with the queue's
    /// two lists and the runs it took the record from beside it.
    fn of(latest: &Latest<FreezeActivity>) -> Self {
        Self {
            waiting: named_folders(&latest.waiting),
            dropped: named_folders(&latest.dropped),
            displaced: latest.displaced.iter().map(Self::alone).collect(),
            ..Self::alone(&latest.activity)
        }
    }

    /// One run and what it came to, with nothing of the flow around it.
    ///
    /// What a displaced run is answered as, and the half of the run on record
    /// that is about the run rather than about the queue. The queue's lists are
    /// empty here because they are not this run's to report: what is waiting and
    /// what was thrown away belong to the flow, and are said once, on the run the
    /// flow is on.
    fn alone(activity: &FreezeActivity) -> Self {
        Self {
            run: activity.run,
            folder: activity.folder.as_str().to_owned(),
            status: activity.status.as_str(),
            packs: activity.packs,
            entries: activity.entries,
            findings: activity.findings.iter().map(FindingDto::of).collect(),
            step: activity.step.as_ref().map(StepDto::of),
            waiting: Vec::new(),
            dropped: Vec::new(),
            displaced: Vec::new(),
            stopped: activity.stopped.as_ref().map(RefusalDto::of),
        }
    }
}

/// Folders as a listing spells them: the Library root is the empty string.
fn named_folders(folders: &[Folder]) -> Vec<String> {
    folders
        .iter()
        .map(|folder| folder.as_str().to_owned())
        .collect()
}

impl FindingDto {
    fn of(finding: &Finding) -> Self {
        Self {
            path: finding.path.clone(),
            message: finding.message.clone(),
            reason: finding.reason,
            surfaced: finding.surfaced,
        }
    }
}

impl FillDto {
    /// The whole of what the flow has to say: the run on record, with the queue's
    /// two lists and the runs it took the record from beside it.
    fn of(latest: &Latest<Activity>) -> Self {
        Self {
            waiting: named_folders(&latest.waiting),
            dropped: named_folders(&latest.dropped),
            displaced: latest.displaced.iter().map(Self::alone).collect(),
            ..Self::alone(&latest.activity)
        }
    }

    /// One run and what it came to, with nothing of the flow around it.
    ///
    /// What a displaced run is answered as, on the terms a freeze's is: the
    /// queue's lists are not this run's to report.
    fn alone(activity: &Activity) -> Self {
        Self {
            run: activity.run,
            folder: activity.folder.as_str().to_owned(),
            status: activity.status.as_str(),
            total: activity.total,
            done: activity.done,
            declined: activity.declined.iter().map(DeclinedDto::of).collect(),
            waiting: Vec::new(),
            dropped: Vec::new(),
            displaced: Vec::new(),
            stopped: activity.stopped.as_ref().map(RefusalDto::of),
        }
    }
}

impl DeclinedDto {
    fn of(declined: &Declined) -> Self {
        Self {
            path: declined.path.clone(),
            refusal: RefusalDto::of(&declined.refusal),
        }
    }
}

impl RefusalDto {
    pub(super) fn of(refusal: &Reported) -> Self {
        Self {
            error: refusal.kind,
            message: refusal.message.clone(),
            reason: refusal.reason,
            surfaced: refusal.surfaced,
        }
    }
}

/// `GET /api/activity`
///
/// Polled while something is happening and not otherwise: an explorer with
/// nothing in flight asks for nothing. An open reader counts as something
/// happening, which is what carries the lock's news to the one screen holding
/// plaintext.
///
/// It needs no key and takes none, so it answers a locked server as readily as
/// an open one.
pub async fn activity(State(state): State<Arc<ServerState>>) -> Json<ActivityDto> {
    Json(ActivityDto::of(&state))
}

// Every state this answer can be in, written to the file the explorer's own
// cases read back through its types.
#[cfg(test)]
mod contract;
