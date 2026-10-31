use serde::Serialize;

use crate::pending_rows::TurnTaker;
use crate::state::ServerState;

use super::{
    AddDto, CatalogDto, DeleteDto, FillDto, FreezeDto, KeptParcelsDto, LoggedDto, ReconnectDto,
    SyncDto,
};

/// What the server is doing on its own, which is four things — and the one
/// thing it may have done to itself.
///
/// A fill, a sync, a freeze and a deletion. Everything else this server does it
/// does because a request asked it to, and a request is answered rather than
/// reported on; these four are the work that goes on after the request that
/// set it going has been answered — the rest of a folder being brought over
/// behind a reader, files somebody dropped being carried into the Library, a
/// book somebody brought in being packed, and files somebody chose being taken
/// out of it — so they are what there is to tell a browser about.
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
/// process answered. The run numbers below it are true of one process only,
/// because they start again at 1 with the next one.
#[derive(Serialize)]
pub struct WorkDto {
    /// What the process that answered calls itself.
    ///
    /// The same string for every answer this process gives and a different one
    /// after it is started again, which is all a page needs from it to tell
    /// one process's answers from the next's. The run numbers cannot tell it:
    /// they start again at 1 with every process, so run 1 of a new server reads
    /// like run 1 of the one before it.
    ///
    /// It says nothing about this device or this Library: see
    /// [`ServerId`](crate::server_id::ServerId) for what it is composed of and
    /// why it is composed rather than taken from something that was already
    /// there.
    pub(super) server: String,
    /// Which of the two states this device holds the Library in, in the words
    /// DK-1 uses: `locked` or `unlocked`.
    pub(super) library: &'static str,
    /// How this device's catalog stands with the Library.
    pub(super) catalog: CatalogDto,
    /// The parcels this device keeps for the Library, and `null` while it is
    /// locked or when they could not be counted: they are counted from the
    /// Index's held-parcel rows, which a locked server does not hold open, and
    /// a failed count is said in the server's log.
    ///
    /// Not work either, and here for the reason the catalog's standing is: it
    /// is device state nothing else tells a person about, and this is the
    /// question an open explorer is already asking (spec: PK-21).
    pub(super) kept_parcels: Option<KeptParcelsDto>,
    /// Which flow's run holds this device's pending rows right now — `sync`,
    /// `freeze` or `delete` — and `null` where nobody does.
    ///
    /// The three take turns at those rows, in the order their runs ask for
    /// them, and a run waiting for its turn is on record as running with no
    /// step, exactly as one that has just taken it is. Said here because a
    /// browser cannot tell the two apart on its own: guessing, it can have two
    /// runs each claim to be waiting for the other. Each run's own `waits` says
    /// whether it is one of those waiting, and this is whom it waits for.
    pub(super) turn: Option<&'static str>,
    /// The latest fill, running or finished, and `null` where none has run.
    pub(super) fill: Option<FillDto>,
    /// The latest sync, running or finished, and `null` where none has run.
    pub(super) sync: Option<SyncDto>,
    /// The latest freeze, running or finished, and `null` where none has run.
    pub(super) freeze: Option<FreezeDto>,
    /// The latest deletion, running or finished, and `null` where none has
    /// run.
    pub(super) delete: Option<DeleteDto>,
    /// The latest reconnect, waiting or ended, and `null` where none has run.
    ///
    /// Not work on the Library either, and here for the reason the catalog's
    /// standing is: a consent flow ends at a person's browser, on nobody's
    /// clock, and the page that opened the consent page learns how it ended by
    /// asking this.
    pub(super) reconnect: Option<ReconnectDto>,
    /// What the server has finished since it started, newest first: every
    /// fill, sync, freeze and deletion that ended, every folder a queue lost,
    /// and every file a reader was refused other than on a peek or a read ahead or by a locked
    /// Library or an Entry the catalog does not hold — the oldest dropped past a fixed
    /// count, a failure kept past it until the same work ends again, and never
    /// more than a fixed ceiling (spec: LA-12).
    ///
    /// Beside the runs above rather than in place of them: those say what each
    /// flow is doing now, and this is what a page that was closed or looking
    /// elsewhere when a run ended reads back. It rides on the work answer
    /// because every answer that can change it is one: a run ends while a page
    /// polls this, and the presses that start one answer with it.
    pub(super) log: Vec<LoggedDto>,
    /// The adds this server is carrying in — receiving their files, or armed
    /// for the run that carries them — each until its run has ended, or,
    /// stopped, until it has been undone (spec: LA-34, LA-36).
    pub(super) adds: Vec<AddDto>,
}

impl WorkDto {
    /// What the server is doing right now, as the browser is told it.
    ///
    /// Read off the state rather than assembled by each caller: the three routes
    /// that answer with the work answer all answer with the whole of it, and a
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
    ///
    /// The turn is read once, before the runs, and every run's `waits` is
    /// measured against that one reading: read per run, a turn handed over
    /// between two reads could leave two runs each saying the other holds it.
    ///
    /// The kept parcels are counted first, because that is the one part of the
    /// answer that waits on anything: everything else is read in one go after
    /// it, so the runs and the turn they are measured against stay one reading.
    ///
    /// The log is read last. A flow logs a run under the same lock that
    /// publishes its end, so a run this answer reads as over is in the log it
    /// reads after it; read first, the one answer that says a run is over could
    /// leave its entry for a poll that never comes, since a page stops asking
    /// once nothing is running.
    pub async fn of(state: &ServerState) -> Self {
        let kept_parcels = state.kept_tally().await.map(KeptParcelsDto::of);
        let turn = state.pending_rows.holder();
        Self {
            server: state.server().to_owned(),
            library: if state.holds_library() {
                "unlocked"
            } else {
                "locked"
            },
            catalog: CatalogDto::of(&state.catalog.standing()),
            kept_parcels,
            turn: turn.map(TurnTaker::as_str),
            fill: state.fills.reported().as_ref().map(FillDto::of),
            sync: state
                .syncs
                .reported()
                .as_ref()
                .map(|run| SyncDto::of(run, turn)),
            freeze: state
                .freezes
                .reported()
                .as_ref()
                .map(|latest| FreezeDto::of(latest, turn)),
            delete: state
                .deletes
                .reported()
                .as_ref()
                .map(|report| DeleteDto::of(report, turn)),
            reconnect: state.reconnects.reported().as_ref().map(ReconnectDto::of),
            // Before the log, for the log's own reason: an add is listed until
            // it has been logged, so read first, one that has just been undone
            // is in one or the other.
            adds: state.adds.listed().iter().map(AddDto::of).collect(),
            log: state.activity.entries().iter().map(LoggedDto::of).collect(),
        }
    }
}
