use crate::reported::Reported;

/// Where a fill stands.
///
/// Four states and the whole set is named here, because a browser writes a
/// branch per state and one it has never heard of is one it falls off the end
/// of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FillStatus {
    /// Armed or walking the folder.
    Filling,
    /// Every file it set out to bring over is here or accounted for.
    Done,
    /// It stopped short, and this says what stopped it: Storage, or the worker
    /// itself ending without an answer.
    ///
    /// The refusal rides on the state rather than beside it, so a run cannot be
    /// stopped without saying why, nor say why without being stopped.
    Stopped(Reported),
    /// A fetch landed in another folder and the fill followed the person there.
    ///
    /// The folder is not taken up again on its own: clicking back into it arms
    /// it afresh.
    Superseded,
}

impl FillStatus {
    /// The word this travels under.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Filling => "filling",
            Self::Done => "done",
            Self::Stopped(_) => "stopped",
            Self::Superseded => "superseded",
        }
    }

    /// What stopped the fill, and `None` where nothing did.
    pub fn stopped(&self) -> Option<&Reported> {
        match self {
            Self::Stopped(refusal) => Some(refusal),
            _ => None,
        }
    }
}
