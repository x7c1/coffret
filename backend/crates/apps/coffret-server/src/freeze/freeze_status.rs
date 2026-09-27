use crate::reported::Reported;

/// Where a freeze stands.
///
/// Three states and the whole set is named here, because a browser writes a
/// branch per state and one it has never heard of is one it falls off the end
/// of.
///
/// There is no `superseded` among them, and there must not be. A fill follows
/// whoever is clicking because the folder they left is still exactly as it was.
/// A freeze commits one batch (spec: PK-7), so one abandoned half way brings in
/// none of its book: the pages stay in the folder with nothing on record
/// about them but the run that superseded that freeze. A second folder waits
/// its turn rather than taking the running one's place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FreezeStatus {
    /// Armed, or packing the folder.
    Freezing,
    /// It finished, whatever it found.
    Done,
    /// It stopped short, and this says what stopped it: Storage, or the worker
    /// itself ending without an answer.
    ///
    /// The refusal rides on the state rather than beside it, so a run cannot be
    /// stopped without saying why, nor say why without being stopped.
    Stopped(Reported),
}

impl FreezeStatus {
    /// The word this travels under.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Freezing => "freezing",
            Self::Done => "done",
            Self::Stopped(_) => "stopped",
        }
    }

    /// What stopped the freeze, and `None` where nothing did.
    pub fn stopped(&self) -> Option<&Reported> {
        match self {
            Self::Stopped(refusal) => Some(refusal),
            _ => None,
        }
    }
}
