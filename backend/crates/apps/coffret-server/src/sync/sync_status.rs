use crate::reported::Reported;

/// Where a sync stands.
///
/// Four states and the whole set is named here, because a browser writes a
/// branch per state and one it has never heard of is one it falls off the end
/// of.
///
/// There is no `superseded` among them, and there is nothing to add one for: a
/// sync covers the device's mappings entire, so a second one asks for what the
/// first is already doing rather than for somewhere else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    /// Armed, or walking the mapped folders.
    Syncing,
    /// It finished, whatever it found.
    Done,
    /// It stopped short, and this says what stopped it: Storage, or the worker
    /// itself ending without an answer.
    ///
    /// The refusal rides on the state rather than beside it, so a run cannot be
    /// stopped without saying why, nor say why without being stopped.
    Stopped(Reported),
    /// A person stopped it, through an add it carried, before it began
    /// committing (spec: LA-35): it committed nothing, and the adds it undid
    /// are each logged in its place, as adds a person stopped.
    Cancelled,
}

impl SyncStatus {
    /// The word this travels under.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Syncing => "syncing",
            Self::Done => "done",
            Self::Stopped(_) => "stopped",
            Self::Cancelled => "cancelled",
        }
    }

    /// What stopped the sync, and `None` where nothing did.
    pub fn stopped(&self) -> Option<&Reported> {
        match self {
            Self::Stopped(refusal) => Some(refusal),
            _ => None,
        }
    }
}
