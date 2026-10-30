use crate::reported::Reported;

/// How a move ended.
///
/// Two states and the whole set is named here, for the reason every flow's is:
/// a browser writes a branch per state. There is no running state, because a
/// move answers the request that asked for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveStatus {
    /// Its record committed, whatever became of this device's files.
    Done,
    /// It stopped short of the commit, and this says what stopped it. Nothing
    /// was committed (spec: CP-1): every Entry it named stands where it was.
    Stopped(Reported),
}

impl MoveStatus {
    /// The word this travels under.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Stopped(_) => "stopped",
        }
    }

    /// What stopped the move, and `None` where nothing did.
    pub fn stopped(&self) -> Option<&Reported> {
        match self {
            Self::Stopped(refusal) => Some(refusal),
            Self::Done => None,
        }
    }
}
