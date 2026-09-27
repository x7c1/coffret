use serde::Serialize;

use crate::state::ServerState;

use super::{CatalogDto, FillDto, FreezeDto, SyncDto};

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
pub struct WorkDto {
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
    pub(super) server: String,
    /// Which of the two states this device holds the Library in, in the words
    /// DK-1 uses: `locked` or `unlocked`.
    pub(super) library: &'static str,
    /// How this device's catalog stands with the Library.
    pub(super) catalog: CatalogDto,
    /// The latest fill, running or finished, and `null` where none has run.
    pub(super) fill: Option<FillDto>,
    /// The latest sync, running or finished, and `null` where none has run.
    pub(super) sync: Option<SyncDto>,
    /// The latest freeze, running or finished, and `null` where none has run.
    pub(super) freeze: Option<FreezeDto>,
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
            sync: state.syncs.reported().as_ref().map(SyncDto::of),
            freeze: state.freezes.reported().as_ref().map(FreezeDto::of),
        }
    }
}
