use crate::reported::Reported;

/// Where a deletion stands.
///
/// Three states and the whole set is named here, because a browser writes a
/// branch per state and one it has never heard of is one it falls off the end
/// of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteStatus {
    /// Armed, or deleting.
    Deleting,
    /// It finished, whatever it was refused.
    Done,
    /// It stopped short, and this says what stopped it. A deletion that stopped
    /// committed nothing (spec: CP-1): every Entry it named is still in the
    /// Library.
    Stopped(Reported),
}

impl DeleteStatus {
    /// The word this travels under.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Deleting => "deleting",
            Self::Done => "done",
            Self::Stopped(_) => "stopped",
        }
    }

    /// What stopped the deletion, and `None` where nothing did.
    pub fn stopped(&self) -> Option<&Reported> {
        match self {
            Self::Stopped(refusal) => Some(refusal),
            _ => None,
        }
    }
}
