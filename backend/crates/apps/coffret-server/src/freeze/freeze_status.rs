/// Where a freeze stands.
///
/// Three states and the whole set is named here, because a browser writes a
/// branch per state and one it has never heard of is one it falls off the end
/// of.
///
/// There is no `superseded` among them, and there must not be. A fill follows
/// whoever is clicking because the folder they left is still exactly as it was.
/// A freeze commits one batch (spec: PK-7), so one abandoned half way brings in
/// no part of its book: the pages stay in the folder with nothing on record
/// about them but the run that displaced them. A second folder waits its turn
/// rather than taking the running one's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreezeStatus {
    /// Armed, or packing the folder.
    Freezing,
    /// It finished, whatever it found.
    Done,
    /// It stopped short, and
    /// [`FreezeActivity::stopped`](super::FreezeActivity::stopped) says what
    /// stopped it: Storage, or the worker itself ending without an answer.
    Stopped,
}

impl FreezeStatus {
    /// The word this travels under.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Freezing => "freezing",
            Self::Done => "done",
            Self::Stopped => "stopped",
        }
    }
}
