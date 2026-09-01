/// Where a sync stands.
///
/// Three states and the whole set is named here, because a browser writes a
/// branch per state and one it has never heard of is one it falls off the end
/// of.
///
/// There is no `superseded` among them, and there is nothing to add one for: a
/// sync covers the device's mappings entire, so a second one asks for what the
/// first is already doing rather than for somewhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    /// Armed, or walking the mapped folders.
    Syncing,
    /// It finished, whatever it found.
    Done,
    /// It stopped short, and
    /// [`SyncActivity::stopped`](super::SyncActivity::stopped) says what stopped
    /// it: Storage, or the worker itself ending without an answer.
    Stopped,
}

impl SyncStatus {
    /// The word this travels under.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Syncing => "syncing",
            Self::Done => "done",
            Self::Stopped => "stopped",
        }
    }
}
