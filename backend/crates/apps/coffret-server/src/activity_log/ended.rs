use crate::adds::StoppedAdd;
use crate::delete::{DeleteRun, DeleteStatus};
use crate::fill::{FillRun, FillStatus};
use crate::folder::Folder;
use crate::freeze::{FreezeRun, FreezeStatus};
use crate::moves::{MoveRun, MoveStatus};
use crate::reported::Reported;
use crate::sync::{SyncRun, SyncStatus};

use super::{CaughtUp, Queue};

/// What one entry is about.
#[derive(Clone, Debug)]
pub enum Ended {
    /// A fill that ended, done or stopped. A fill superseded by another folder
    /// did not end: its folder is left for the next page opened in it.
    Fill(FillRun),
    /// A sync that ended.
    Sync(SyncRun),
    /// A freeze that ended.
    Freeze(FreezeRun),
    /// A deletion that ended.
    Delete(DeleteRun),
    /// A move that went past its refusals, committed or stopped short of the
    /// commit. One refused before its commit — by the set, whether the preview
    /// or the rename's own plan refuses it, an edited file or one still
    /// uploading — did nothing, and is answered rather than logged.
    Move(MoveRun),
    /// An add a person stopped, and what undoing it on this device came to
    /// (spec: LA-36). A sync or a freeze stopped through the adds it carried
    /// is logged as those adds and not as itself, save a freeze whose book
    /// somebody had also asked to have packed, or that was still owed a pack
    /// since a run of it stopped, stopped on its line, which is logged
    /// `cancelled` beside them (spec: LA-12).
    Add(StoppedAdd),
    /// A catch-up the explorer asked for (`POST /api/refresh`: *Check for
    /// changes*), and what it came to: the numbers it landed with, or what
    /// stopped it. One the server made on its own — as it starts, after a
    /// reconnect, or for a fetch that met a stale catalog — is not logged, and
    /// neither is one a locked Library refused before it began (spec: LA-12).
    CatchUp(Result<CaughtUp, Reported>),
    /// A folder a queue threw away when its worker ended without an answer.
    Discarded {
        /// Which queue lost it.
        queue: Queue,
        /// The folder it lost.
        folder: Folder,
    },
    /// A file a reader asked for and was refused — by anything but a locked
    /// Library or an Entry the catalog does not hold, which the file route
    /// leaves out of the log, as it leaves out every peek and every read ahead.
    Fetch {
        /// The Entry Path it asked for.
        path: String,
        /// What it was refused with.
        refusal: Reported,
    },
}

/// What "the same work" is, for one entry settling another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Key {
    Fill(Folder),
    Freeze(Folder),
    Sync,
    CatchUp,
    Delete(Vec<String>, Vec<String>),
    Move(Vec<(String, String)>),
    Fetch(String),
    Add(Folder),
}

impl Ended {
    /// Whether this is a failure: a run that stopped, a folder that never
    /// started, a fetch or a catch-up that was refused.
    pub fn failed(&self) -> bool {
        match self {
            Self::Fill(run) => matches!(run.status, FillStatus::Stopped(_)),
            Self::Sync(run) => matches!(run.status, SyncStatus::Stopped(_)),
            Self::Freeze(run) => matches!(run.status, FreezeStatus::Stopped(_)),
            Self::Delete(run) => matches!(run.status, DeleteStatus::Stopped(_)),
            Self::Move(run) => matches!(run.status, MoveStatus::Stopped(_)),
            Self::CatchUp(outcome) => outcome.is_err(),
            Self::Discarded { .. } | Self::Fetch { .. } => true,
            // What a person asked for, and so no failure of anything.
            Self::Add(_) => false,
        }
    }

    /// The work this entry is about, which a later entry about the same work
    /// settles.
    ///
    /// A folder a queue lost is the folder's fill or freeze, so the next run on
    /// that folder settles it as it settles a run that stopped there.
    pub(super) fn key(&self) -> Key {
        match self {
            Self::Fill(run) => Key::Fill(run.folder.clone()),
            Self::Freeze(run) => Key::Freeze(run.folder.clone()),
            Self::Discarded {
                queue: Queue::Fill,
                folder,
            } => Key::Fill(folder.clone()),
            Self::Discarded {
                queue: Queue::Freeze,
                folder,
            } => Key::Freeze(folder.clone()),
            Self::Sync(_) => Key::Sync,
            Self::CatchUp(_) => Key::CatchUp,
            Self::Delete(run) => Key::Delete(run.target.folders_named(), run.target.paths_named()),
            Self::Move(run) => Key::Move(run.pairs_named()),
            Self::Fetch { path, .. } => Key::Fetch(path.clone()),
            Self::Add(stopped) => Key::Add(stopped.folder.clone()),
        }
    }
}
