use crate::delete::{DeleteRun, DeleteStatus};
use crate::fill::{FillRun, FillStatus};
use crate::folder::Folder;
use crate::freeze::{FreezeRun, FreezeStatus};
use crate::reported::Reported;
use crate::sync::{SyncRun, SyncStatus};

use super::Queue;

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
    /// A folder a queue threw away when its worker ended without an answer.
    Discarded {
        /// Which queue lost it.
        queue: Queue,
        /// The folder it lost.
        folder: Folder,
    },
    /// A file a reader asked for and was refused — by anything but a locked
    /// Library or an Entry the catalog does not hold, which the file route
    /// leaves out of the log, as it leaves out every peek.
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
    Delete(Vec<String>, Vec<String>),
    Fetch(String),
}

impl Ended {
    /// Whether this is a failure: a run that stopped, a folder that never
    /// started, a fetch that was refused.
    pub fn failed(&self) -> bool {
        match self {
            Self::Fill(run) => matches!(run.status, FillStatus::Stopped(_)),
            Self::Sync(run) => matches!(run.status, SyncStatus::Stopped(_)),
            Self::Freeze(run) => matches!(run.status, FreezeStatus::Stopped(_)),
            Self::Delete(run) => matches!(run.status, DeleteStatus::Stopped(_)),
            Self::Discarded { .. } | Self::Fetch { .. } => true,
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
            Self::Delete(run) => Key::Delete(run.target.folders_named(), run.target.paths_named()),
            Self::Fetch { path, .. } => Key::Fetch(path.clone()),
        }
    }
}
